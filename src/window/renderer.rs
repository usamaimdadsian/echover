//! Vulkan renderer using `ash`.
//!
//! Single-frame-in-flight, single graphics queue, FIFO swapchain,
//! one render pass + framebuffer per swapchain image, one pipeline that
//! branches in the fragment shader on `DrawKind` (0 = SDF rounded rect,
//! 1 = sample the R8 glyph atlas). The atlas is a 1024×1024 image baked
//! once at init from the `Font::Atlas` we already build on the CPU.
//!
//! Vertex data is written into a single host-visible vertex buffer each
//! frame (no index buffer — six verts per quad keeps the upload code
//! straightforward and the geometry tiny).

use std::ffi::{CStr, CString};
use std::mem::size_of;

use ash::khr;
use ash::vk;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::ui::font::Atlas;
use crate::ui::primitives::{DrawCommand, DrawKind};

const FRAME_OVERLAP: usize = 1;
const ATLAS_FORMAT: vk::Format = vk::Format::R8_UNORM;
const SWAPCHAIN_FORMAT: vk::Format = vk::Format::B8G8R8A8_UNORM;
const SWAPCHAIN_COLORSPACE: vk::ColorSpaceKHR = vk::ColorSpaceKHR::SRGB_NONLINEAR;
/// 1px feather around solid rounded rects so SDF anti-aliasing has room.
const AA_PAD: f32 = 1.0;
/// Bytes per vertex (matches `ui.vert` attribute layout).
const VERTEX_STRIDE: u32 = 56;
/// Soft cap on quads per frame; enough headroom for a busy page.
const MAX_QUADS: usize = 8192;

#[repr(C)]
#[derive(Clone, Copy)]
struct UiVertex {
    pos: [f32; 2],            // loc 0
    color: [f32; 4],          // loc 1
    rect_min: [f32; 2],       // loc 2
    rect_max: [f32; 2],       // loc 3
    radius: f32,              // loc 4
    kind: u32,                // loc 5
    uv: [f32; 2],             // loc 6
}

pub struct Renderer {
    // Pure-Vulkan handles. `Drop` cleans them up in reverse order.
    _entry: ash::Entry,
    instance: ash::Instance,
    surface_loader: khr::surface::Instance,
    surface: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
    queue_family: u32,
    device: ash::Device,
    queue: vk::Queue,
    swapchain_loader: khr::swapchain::Device,
    swapchain: vk::SwapchainKHR,
    swapchain_extent: vk::Extent2D,
    swapchain_images: Vec<vk::Image>,
    swapchain_views: Vec<vk::ImageView>,
    render_pass: vk::RenderPass,
    framebuffers: Vec<vk::Framebuffer>,

    descriptor_set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set: vk::DescriptorSet,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    vert_module: vk::ShaderModule,
    frag_module: vk::ShaderModule,

    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
    image_available: vk::Semaphore,
    render_finished: vk::Semaphore,
    in_flight: vk::Fence,

    // Host-visible vertex buffer, mapped persistently.
    vertex_buffer: vk::Buffer,
    vertex_memory: vk::DeviceMemory,
    vertex_mapped: *mut u8,
    vertex_capacity_bytes: usize,

    // Atlas resources.
    atlas_image: vk::Image,
    atlas_memory: vk::DeviceMemory,
    atlas_view: vk::ImageView,
    atlas_sampler: vk::Sampler,

    background: [f32; 4],
}

unsafe impl Send for Renderer {}

impl Renderer {
    pub fn new(
        window: &winit::window::Window,
        width: u32,
        height: u32,
        atlas: &Atlas,
    ) -> Result<Self, String> {
        unsafe {
            let entry = ash::Entry::load()
                .map_err(|error| format!("failed to load Vulkan loader: {error}"))?;

            let app_name = CString::new("Echover").unwrap();
            let app_info = vk::ApplicationInfo::default()
                .application_name(&app_name)
                .application_version(vk::make_api_version(0, 0, 1, 0))
                .engine_name(&app_name)
                .engine_version(vk::make_api_version(0, 0, 1, 0))
                .api_version(vk::API_VERSION_1_2);

            let display_handle = window
                .display_handle()
                .map_err(|error| format!("display handle: {error}"))?
                .as_raw();
            let mut required_exts =
                ash_window::enumerate_required_extensions(display_handle)
                    .map_err(|error| format!("required surface extensions: {error}"))?
                    .to_vec();

            // Some Linux compositors require this on top of the platform surface ext.
            required_exts.push(khr::get_physical_device_properties2::NAME.as_ptr());

            let instance_info = vk::InstanceCreateInfo::default()
                .application_info(&app_info)
                .enabled_extension_names(&required_exts);
            let instance = entry
                .create_instance(&instance_info, None)
                .map_err(|error| format!("create_instance failed: {error}"))?;

            let window_handle = window
                .window_handle()
                .map_err(|error| format!("window handle: {error}"))?
                .as_raw();
            let surface = ash_window::create_surface(
                &entry,
                &instance,
                display_handle,
                window_handle,
                None,
            )
            .map_err(|error| format!("create_surface failed: {error}"))?;
            let surface_loader = khr::surface::Instance::new(&entry, &instance);

            let (physical_device, queue_family) =
                pick_physical_device(&instance, &surface_loader, surface)?;

            let queue_priorities = [1.0_f32];
            let queue_info = vk::DeviceQueueCreateInfo::default()
                .queue_family_index(queue_family)
                .queue_priorities(&queue_priorities);
            let queue_infos = [queue_info];
            let device_exts = [khr::swapchain::NAME.as_ptr()];
            let device_features = vk::PhysicalDeviceFeatures::default();
            let device_info = vk::DeviceCreateInfo::default()
                .queue_create_infos(&queue_infos)
                .enabled_extension_names(&device_exts)
                .enabled_features(&device_features);
            let device = instance
                .create_device(physical_device, &device_info, None)
                .map_err(|error| format!("create_device failed: {error}"))?;
            let queue = device.get_device_queue(queue_family, 0);

            let swapchain_loader = khr::swapchain::Device::new(&instance, &device);

            let SwapchainBundle {
                swapchain,
                extent,
                images,
                views,
                render_pass,
                framebuffers,
            } = create_swapchain_and_framebuffers(
                &surface_loader,
                surface,
                &swapchain_loader,
                &device,
                physical_device,
                width,
                height,
                vk::SwapchainKHR::null(),
            )?;

            // Atlas image + view + sampler, then upload pixels.
            let (atlas_image, atlas_memory, atlas_view, atlas_sampler) =
                create_atlas(&instance, physical_device, &device, queue, queue_family, atlas)?;

            // Descriptor set + layout for the atlas binding.
            let descriptor_set_layout = create_descriptor_set_layout(&device)?;
            let descriptor_pool = create_descriptor_pool(&device)?;
            let descriptor_set = allocate_descriptor_set(
                &device,
                descriptor_pool,
                descriptor_set_layout,
                atlas_view,
                atlas_sampler,
            )?;

            // Pipeline.
            let (vert_module, frag_module) = load_shader_modules(&device)?;
            let pipeline_layout = create_pipeline_layout(&device, descriptor_set_layout)?;
            let pipeline = create_pipeline(
                &device,
                pipeline_layout,
                render_pass,
                vert_module,
                frag_module,
            )?;

            // Command pool / buffer.
            let pool_info = vk::CommandPoolCreateInfo::default()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(queue_family);
            let command_pool = device
                .create_command_pool(&pool_info, None)
                .map_err(|error| format!("create_command_pool failed: {error}"))?;
            let cb_info = vk::CommandBufferAllocateInfo::default()
                .command_pool(command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(FRAME_OVERLAP as u32);
            let command_buffer = device
                .allocate_command_buffers(&cb_info)
                .map_err(|error| format!("allocate_command_buffers failed: {error}"))?[0];

            // Sync.
            let semaphore_info = vk::SemaphoreCreateInfo::default();
            let image_available = device
                .create_semaphore(&semaphore_info, None)
                .map_err(|error| format!("create_semaphore failed: {error}"))?;
            let render_finished = device
                .create_semaphore(&semaphore_info, None)
                .map_err(|error| format!("create_semaphore failed: {error}"))?;
            let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
            let in_flight = device
                .create_fence(&fence_info, None)
                .map_err(|error| format!("create_fence failed: {error}"))?;

            // Vertex buffer (host-visible, persistently mapped).
            let vertex_capacity_bytes = MAX_QUADS * 6 * size_of::<UiVertex>();
            let (vertex_buffer, vertex_memory) = create_buffer(
                &instance,
                physical_device,
                &device,
                vertex_capacity_bytes as vk::DeviceSize,
                vk::BufferUsageFlags::VERTEX_BUFFER,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            )?;
            let vertex_mapped = device
                .map_memory(
                    vertex_memory,
                    0,
                    vertex_capacity_bytes as vk::DeviceSize,
                    vk::MemoryMapFlags::empty(),
                )
                .map_err(|error| format!("map_memory failed: {error}"))?
                as *mut u8;

            Ok(Self {
                _entry: entry,
                instance,
                surface_loader,
                surface,
                physical_device,
                queue_family,
                device,
                queue,
                swapchain_loader,
                swapchain,
                swapchain_extent: extent,
                swapchain_images: images,
                swapchain_views: views,
                render_pass,
                framebuffers,
                descriptor_set_layout,
                descriptor_pool,
                descriptor_set,
                pipeline_layout,
                pipeline,
                vert_module,
                frag_module,
                command_pool,
                command_buffer,
                image_available,
                render_finished,
                in_flight,
                vertex_buffer,
                vertex_memory,
                vertex_mapped,
                vertex_capacity_bytes,
                atlas_image,
                atlas_memory,
                atlas_view,
                atlas_sampler,
                background: [0.929, 0.910, 0.882, 1.0],
            })
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        unsafe {
            let _ = self.device.device_wait_idle();
            for fb in self.framebuffers.drain(..) {
                self.device.destroy_framebuffer(fb, None);
            }
            for view in self.swapchain_views.drain(..) {
                self.device.destroy_image_view(view, None);
            }
            self.device.destroy_render_pass(self.render_pass, None);

            let bundle = create_swapchain_and_framebuffers(
                &self.surface_loader,
                self.surface,
                &self.swapchain_loader,
                &self.device,
                self.physical_device,
                width,
                height,
                self.swapchain,
            )?;
            self.swapchain_loader.destroy_swapchain(self.swapchain, None);
            self.swapchain = bundle.swapchain;
            self.swapchain_extent = bundle.extent;
            self.swapchain_images = bundle.images;
            self.swapchain_views = bundle.views;
            self.render_pass = bundle.render_pass;
            self.framebuffers = bundle.framebuffers;

            // Pipeline references the render pass — recreate.
            self.device.destroy_pipeline(self.pipeline, None);
            self.pipeline = create_pipeline(
                &self.device,
                self.pipeline_layout,
                self.render_pass,
                self.vert_module,
                self.frag_module,
            )?;
        }
        Ok(())
    }

    pub fn render(
        &mut self,
        width: u32,
        height: u32,
        commands: &[DrawCommand],
    ) -> Result<(), String> {
        if width == 0 || height == 0 {
            return Ok(());
        }

        // Build vertex stream on the CPU. Cap at MAX_QUADS to avoid overflow.
        let mut verts: Vec<UiVertex> = Vec::with_capacity(commands.len() * 6);
        for command in commands.iter().take(MAX_QUADS) {
            push_quad(&mut verts, command);
        }
        let vertex_count = verts.len() as u32;
        let bytes = verts.len() * size_of::<UiVertex>();
        if bytes > self.vertex_capacity_bytes {
            return Err(format!(
                "vertex buffer overflow: {bytes} > {}",
                self.vertex_capacity_bytes
            ));
        }

        unsafe {
            // Copy verts into mapped memory.
            if !verts.is_empty() {
                std::ptr::copy_nonoverlapping(
                    verts.as_ptr() as *const u8,
                    self.vertex_mapped,
                    bytes,
                );
            }

            self.device
                .wait_for_fences(&[self.in_flight], true, u64::MAX)
                .map_err(|error| format!("wait_for_fences failed: {error}"))?;

            let acquire = self.swapchain_loader.acquire_next_image(
                self.swapchain,
                u64::MAX,
                self.image_available,
                vk::Fence::null(),
            );
            let image_index = match acquire {
                Ok((idx, _suboptimal)) => idx,
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    return self.resize(width, height);
                }
                Err(error) => {
                    return Err(format!("acquire_next_image failed: {error}"));
                }
            };

            self.device
                .reset_fences(&[self.in_flight])
                .map_err(|error| format!("reset_fences failed: {error}"))?;

            self.device
                .reset_command_buffer(
                    self.command_buffer,
                    vk::CommandBufferResetFlags::empty(),
                )
                .map_err(|error| format!("reset_command_buffer failed: {error}"))?;

            let begin = vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
            self.device
                .begin_command_buffer(self.command_buffer, &begin)
                .map_err(|error| format!("begin_command_buffer failed: {error}"))?;

            let clear = [vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: self.background,
                },
            }];
            let rp_begin = vk::RenderPassBeginInfo::default()
                .render_pass(self.render_pass)
                .framebuffer(self.framebuffers[image_index as usize])
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: self.swapchain_extent,
                })
                .clear_values(&clear);
            self.device.cmd_begin_render_pass(
                self.command_buffer,
                &rp_begin,
                vk::SubpassContents::INLINE,
            );

            if vertex_count > 0 {
                self.device.cmd_bind_pipeline(
                    self.command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipeline,
                );
                self.device.cmd_bind_descriptor_sets(
                    self.command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipeline_layout,
                    0,
                    &[self.descriptor_set],
                    &[],
                );

                let viewport = vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: self.swapchain_extent.width as f32,
                    height: self.swapchain_extent.height as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                };
                self.device.cmd_set_viewport(self.command_buffer, 0, &[viewport]);
                let scissor = vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: self.swapchain_extent,
                };
                self.device.cmd_set_scissor(self.command_buffer, 0, &[scissor]);

                let push = [
                    self.swapchain_extent.width as f32,
                    self.swapchain_extent.height as f32,
                ];
                let push_bytes = std::slice::from_raw_parts(
                    push.as_ptr() as *const u8,
                    size_of::<[f32; 2]>(),
                );
                self.device.cmd_push_constants(
                    self.command_buffer,
                    self.pipeline_layout,
                    vk::ShaderStageFlags::VERTEX,
                    0,
                    push_bytes,
                );

                self.device.cmd_bind_vertex_buffers(
                    self.command_buffer,
                    0,
                    &[self.vertex_buffer],
                    &[0],
                );
                self.device
                    .cmd_draw(self.command_buffer, vertex_count, 1, 0, 0);
            }

            self.device.cmd_end_render_pass(self.command_buffer);
            self.device
                .end_command_buffer(self.command_buffer)
                .map_err(|error| format!("end_command_buffer failed: {error}"))?;

            let wait = [self.image_available];
            let stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let cbs = [self.command_buffer];
            let signal = [self.render_finished];
            let submit = vk::SubmitInfo::default()
                .wait_semaphores(&wait)
                .wait_dst_stage_mask(&stages)
                .command_buffers(&cbs)
                .signal_semaphores(&signal);
            self.device
                .queue_submit(self.queue, &[submit], self.in_flight)
                .map_err(|error| format!("queue_submit failed: {error}"))?;

            let swapchains = [self.swapchain];
            let indices = [image_index];
            let present = vk::PresentInfoKHR::default()
                .wait_semaphores(&signal)
                .swapchains(&swapchains)
                .image_indices(&indices);
            match self.swapchain_loader.queue_present(self.queue, &present) {
                Ok(_) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(vk::Result::SUBOPTIMAL_KHR) => {}
                Err(error) => return Err(format!("queue_present failed: {error}")),
            }
        }
        Ok(())
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_fence(self.in_flight, None);
            self.device.destroy_semaphore(self.render_finished, None);
            self.device.destroy_semaphore(self.image_available, None);
            self.device.destroy_command_pool(self.command_pool, None);

            self.device.unmap_memory(self.vertex_memory);
            self.device.destroy_buffer(self.vertex_buffer, None);
            self.device.free_memory(self.vertex_memory, None);

            self.device.destroy_sampler(self.atlas_sampler, None);
            self.device.destroy_image_view(self.atlas_view, None);
            self.device.destroy_image(self.atlas_image, None);
            self.device.free_memory(self.atlas_memory, None);

            self.device.destroy_pipeline(self.pipeline, None);
            self.device.destroy_pipeline_layout(self.pipeline_layout, None);
            self.device.destroy_shader_module(self.vert_module, None);
            self.device.destroy_shader_module(self.frag_module, None);
            self.device.destroy_descriptor_pool(self.descriptor_pool, None);
            self.device
                .destroy_descriptor_set_layout(self.descriptor_set_layout, None);

            for fb in self.framebuffers.drain(..) {
                self.device.destroy_framebuffer(fb, None);
            }
            for view in self.swapchain_views.drain(..) {
                self.device.destroy_image_view(view, None);
            }
            self.device.destroy_render_pass(self.render_pass, None);
            self.swapchain_loader.destroy_swapchain(self.swapchain, None);
            self.device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
            // queue_family is not a handle, suppress unused warning.
            let _ = self.queue_family;
            let _ = self.queue;
        }
    }
}

// -- helpers --------------------------------------------------------------

struct SwapchainBundle {
    swapchain: vk::SwapchainKHR,
    extent: vk::Extent2D,
    images: Vec<vk::Image>,
    views: Vec<vk::ImageView>,
    render_pass: vk::RenderPass,
    framebuffers: Vec<vk::Framebuffer>,
}

unsafe fn pick_physical_device(
    instance: &ash::Instance,
    surface_loader: &khr::surface::Instance,
    surface: vk::SurfaceKHR,
) -> Result<(vk::PhysicalDevice, u32), String> {
    let devices = instance
        .enumerate_physical_devices()
        .map_err(|error| format!("enumerate_physical_devices: {error}"))?;
    for device in devices {
        let families = instance.get_physical_device_queue_family_properties(device);
        for (idx, family) in families.iter().enumerate() {
            if !family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                continue;
            }
            let supports_present = surface_loader
                .get_physical_device_surface_support(device, idx as u32, surface)
                .unwrap_or(false);
            if supports_present {
                return Ok((device, idx as u32));
            }
        }
    }
    Err("no Vulkan device with graphics + present queue".to_owned())
}

unsafe fn create_swapchain_and_framebuffers(
    surface_loader: &khr::surface::Instance,
    surface: vk::SurfaceKHR,
    swapchain_loader: &khr::swapchain::Device,
    device: &ash::Device,
    physical_device: vk::PhysicalDevice,
    width: u32,
    height: u32,
    old_swapchain: vk::SwapchainKHR,
) -> Result<SwapchainBundle, String> {
    let caps = surface_loader
        .get_physical_device_surface_capabilities(physical_device, surface)
        .map_err(|error| format!("surface_capabilities: {error}"))?;

    let extent = if caps.current_extent.width == u32::MAX {
        vk::Extent2D {
            width: width.clamp(caps.min_image_extent.width, caps.max_image_extent.width),
            height: height.clamp(caps.min_image_extent.height, caps.max_image_extent.height),
        }
    } else {
        caps.current_extent
    };

    let formats = surface_loader
        .get_physical_device_surface_formats(physical_device, surface)
        .map_err(|error| format!("surface_formats: {error}"))?;
    let surface_format = formats
        .iter()
        .copied()
        .find(|f| f.format == SWAPCHAIN_FORMAT && f.color_space == SWAPCHAIN_COLORSPACE)
        .unwrap_or(formats[0]);

    let mut min_image_count = caps.min_image_count + 1;
    if caps.max_image_count != 0 && min_image_count > caps.max_image_count {
        min_image_count = caps.max_image_count;
    }

    let info = vk::SwapchainCreateInfoKHR::default()
        .surface(surface)
        .min_image_count(min_image_count)
        .image_format(surface_format.format)
        .image_color_space(surface_format.color_space)
        .image_extent(extent)
        .image_array_layers(1)
        .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
        .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
        .pre_transform(caps.current_transform)
        .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
        .present_mode(vk::PresentModeKHR::FIFO)
        .clipped(true)
        .old_swapchain(old_swapchain);

    let swapchain = swapchain_loader
        .create_swapchain(&info, None)
        .map_err(|error| format!("create_swapchain: {error}"))?;

    let images = swapchain_loader
        .get_swapchain_images(swapchain)
        .map_err(|error| format!("get_swapchain_images: {error}"))?;

    let views: Vec<vk::ImageView> = images
        .iter()
        .map(|&image| {
            let view_info = vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(surface_format.format)
                .components(vk::ComponentMapping::default())
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });
            device.create_image_view(&view_info, None)
        })
        .collect::<Result<_, _>>()
        .map_err(|error| format!("create_image_view: {error}"))?;

    let color_attachment = vk::AttachmentDescription::default()
        .format(surface_format.format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);
    let color_ref = vk::AttachmentReference::default()
        .attachment(0)
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
    let color_refs = [color_ref];
    let subpass = vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&color_refs);
    let dependency = vk::SubpassDependency::default()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .src_access_mask(vk::AccessFlags::empty())
        .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE);
    let attachments = [color_attachment];
    let subpasses = [subpass];
    let dependencies = [dependency];
    let rp_info = vk::RenderPassCreateInfo::default()
        .attachments(&attachments)
        .subpasses(&subpasses)
        .dependencies(&dependencies);
    let render_pass = device
        .create_render_pass(&rp_info, None)
        .map_err(|error| format!("create_render_pass: {error}"))?;

    let framebuffers: Vec<vk::Framebuffer> = views
        .iter()
        .map(|view| {
            let attachments = [*view];
            let info = vk::FramebufferCreateInfo::default()
                .render_pass(render_pass)
                .attachments(&attachments)
                .width(extent.width)
                .height(extent.height)
                .layers(1);
            device.create_framebuffer(&info, None)
        })
        .collect::<Result<_, _>>()
        .map_err(|error| format!("create_framebuffer: {error}"))?;

    Ok(SwapchainBundle {
        swapchain,
        extent,
        images,
        views,
        render_pass,
        framebuffers,
    })
}

unsafe fn create_descriptor_set_layout(device: &ash::Device) -> Result<vk::DescriptorSetLayout, String> {
    let binding = vk::DescriptorSetLayoutBinding::default()
        .binding(0)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .descriptor_count(1)
        .stage_flags(vk::ShaderStageFlags::FRAGMENT);
    let bindings = [binding];
    let info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
    device
        .create_descriptor_set_layout(&info, None)
        .map_err(|error| format!("create_descriptor_set_layout: {error}"))
}

unsafe fn create_descriptor_pool(device: &ash::Device) -> Result<vk::DescriptorPool, String> {
    let size = vk::DescriptorPoolSize {
        ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
        descriptor_count: 1,
    };
    let sizes = [size];
    let info = vk::DescriptorPoolCreateInfo::default()
        .pool_sizes(&sizes)
        .max_sets(1);
    device
        .create_descriptor_pool(&info, None)
        .map_err(|error| format!("create_descriptor_pool: {error}"))
}

unsafe fn allocate_descriptor_set(
    device: &ash::Device,
    pool: vk::DescriptorPool,
    layout: vk::DescriptorSetLayout,
    view: vk::ImageView,
    sampler: vk::Sampler,
) -> Result<vk::DescriptorSet, String> {
    let layouts = [layout];
    let alloc = vk::DescriptorSetAllocateInfo::default()
        .descriptor_pool(pool)
        .set_layouts(&layouts);
    let set = device
        .allocate_descriptor_sets(&alloc)
        .map_err(|error| format!("allocate_descriptor_sets: {error}"))?[0];

    let image_info = vk::DescriptorImageInfo::default()
        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
        .image_view(view)
        .sampler(sampler);
    let image_infos = [image_info];
    let write = vk::WriteDescriptorSet::default()
        .dst_set(set)
        .dst_binding(0)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .image_info(&image_infos);
    device.update_descriptor_sets(&[write], &[]);
    Ok(set)
}

unsafe fn load_shader_modules(
    device: &ash::Device,
) -> Result<(vk::ShaderModule, vk::ShaderModule), String> {
    let vert = include_bytes!("../../shaders/ui.vert.spv");
    let frag = include_bytes!("../../shaders/ui.frag.spv");
    Ok((make_module(device, vert)?, make_module(device, frag)?))
}

unsafe fn make_module(device: &ash::Device, bytes: &[u8]) -> Result<vk::ShaderModule, String> {
    if bytes.len() % 4 != 0 {
        return Err(format!("SPIR-V blob length {} not 4-byte aligned", bytes.len()));
    }
    let words: Vec<u32> = bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let info = vk::ShaderModuleCreateInfo::default().code(&words);
    device
        .create_shader_module(&info, None)
        .map_err(|error| format!("create_shader_module: {error}"))
}

unsafe fn create_pipeline_layout(
    device: &ash::Device,
    set_layout: vk::DescriptorSetLayout,
) -> Result<vk::PipelineLayout, String> {
    let push = vk::PushConstantRange::default()
        .stage_flags(vk::ShaderStageFlags::VERTEX)
        .offset(0)
        .size(size_of::<[f32; 2]>() as u32);
    let push_ranges = [push];
    let set_layouts = [set_layout];
    let info = vk::PipelineLayoutCreateInfo::default()
        .set_layouts(&set_layouts)
        .push_constant_ranges(&push_ranges);
    device
        .create_pipeline_layout(&info, None)
        .map_err(|error| format!("create_pipeline_layout: {error}"))
}

unsafe fn create_pipeline(
    device: &ash::Device,
    layout: vk::PipelineLayout,
    render_pass: vk::RenderPass,
    vert: vk::ShaderModule,
    frag: vk::ShaderModule,
) -> Result<vk::Pipeline, String> {
    let main = CString::new("main").unwrap();
    let stages = [
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vert)
            .name(&main),
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(frag)
            .name(&main),
    ];

    // Vertex layout — must match `UiVertex` byte layout.
    let binding = vk::VertexInputBindingDescription::default()
        .binding(0)
        .stride(VERTEX_STRIDE)
        .input_rate(vk::VertexInputRate::VERTEX);
    let bindings = [binding];

    let attrs = [
        // pos: vec2 @ offset 0
        vk::VertexInputAttributeDescription::default()
            .location(0)
            .binding(0)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(0),
        // color: vec4 @ offset 8
        vk::VertexInputAttributeDescription::default()
            .location(1)
            .binding(0)
            .format(vk::Format::R32G32B32A32_SFLOAT)
            .offset(8),
        // rect_min: vec2 @ offset 24
        vk::VertexInputAttributeDescription::default()
            .location(2)
            .binding(0)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(24),
        // rect_max: vec2 @ offset 32
        vk::VertexInputAttributeDescription::default()
            .location(3)
            .binding(0)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(32),
        // radius: float @ offset 40
        vk::VertexInputAttributeDescription::default()
            .location(4)
            .binding(0)
            .format(vk::Format::R32_SFLOAT)
            .offset(40),
        // kind: uint @ offset 44
        vk::VertexInputAttributeDescription::default()
            .location(5)
            .binding(0)
            .format(vk::Format::R32_UINT)
            .offset(44),
        // uv: vec2 @ offset 48
        vk::VertexInputAttributeDescription::default()
            .location(6)
            .binding(0)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(48),
    ];
    let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
        .vertex_binding_descriptions(&bindings)
        .vertex_attribute_descriptions(&attrs);

    let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
        .primitive_restart_enable(false);

    let viewports = [vk::Viewport::default()];
    let scissors = [vk::Rect2D::default()];
    let viewport = vk::PipelineViewportStateCreateInfo::default()
        .viewports(&viewports)
        .scissors(&scissors);
    let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
    let dynamic = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

    let raster = vk::PipelineRasterizationStateCreateInfo::default()
        .polygon_mode(vk::PolygonMode::FILL)
        .cull_mode(vk::CullModeFlags::NONE)
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
        .line_width(1.0);
    let multisample = vk::PipelineMultisampleStateCreateInfo::default()
        .rasterization_samples(vk::SampleCountFlags::TYPE_1);

    let blend_attachment = vk::PipelineColorBlendAttachmentState::default()
        .blend_enable(true)
        .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
        .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .color_blend_op(vk::BlendOp::ADD)
        .src_alpha_blend_factor(vk::BlendFactor::ONE)
        .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .alpha_blend_op(vk::BlendOp::ADD)
        .color_write_mask(vk::ColorComponentFlags::RGBA);
    let blend_attachments = [blend_attachment];
    let blend = vk::PipelineColorBlendStateCreateInfo::default()
        .logic_op_enable(false)
        .attachments(&blend_attachments);

    let info = vk::GraphicsPipelineCreateInfo::default()
        .stages(&stages)
        .vertex_input_state(&vertex_input)
        .input_assembly_state(&input_assembly)
        .viewport_state(&viewport)
        .rasterization_state(&raster)
        .multisample_state(&multisample)
        .color_blend_state(&blend)
        .dynamic_state(&dynamic)
        .layout(layout)
        .render_pass(render_pass)
        .subpass(0);
    let pipelines = device
        .create_graphics_pipelines(vk::PipelineCache::null(), &[info], None)
        .map_err(|(_, error)| format!("create_graphics_pipelines: {error}"))?;
    Ok(pipelines[0])
}

unsafe fn create_atlas(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    device: &ash::Device,
    queue: vk::Queue,
    queue_family: u32,
    atlas: &Atlas,
) -> Result<(vk::Image, vk::DeviceMemory, vk::ImageView, vk::Sampler), String> {
    // 1. Image (DEVICE_LOCAL).
    let image_info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(ATLAS_FORMAT)
        .extent(vk::Extent3D {
            width: atlas.width,
            height: atlas.height,
            depth: 1,
        })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED);
    let image = device
        .create_image(&image_info, None)
        .map_err(|error| format!("create_image atlas: {error}"))?;

    let req = device.get_image_memory_requirements(image);
    let mem_type = find_memory_type(
        instance,
        physical_device,
        req.memory_type_bits,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )?;
    let alloc = vk::MemoryAllocateInfo::default()
        .allocation_size(req.size)
        .memory_type_index(mem_type);
    let memory = device
        .allocate_memory(&alloc, None)
        .map_err(|error| format!("allocate_memory atlas: {error}"))?;
    device
        .bind_image_memory(image, memory, 0)
        .map_err(|error| format!("bind_image_memory atlas: {error}"))?;

    // 2. Staging buffer (HOST_VISIBLE), copy pixels.
    let staging_size = atlas.pixels.len() as vk::DeviceSize;
    let (staging_buffer, staging_memory) = create_buffer(
        instance,
        physical_device,
        device,
        staging_size,
        vk::BufferUsageFlags::TRANSFER_SRC,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )?;
    let mapped = device
        .map_memory(staging_memory, 0, staging_size, vk::MemoryMapFlags::empty())
        .map_err(|error| format!("map staging: {error}"))?;
    std::ptr::copy_nonoverlapping(
        atlas.pixels.as_ptr(),
        mapped as *mut u8,
        atlas.pixels.len(),
    );
    device.unmap_memory(staging_memory);

    // 3. One-shot command buffer: layout transition + copy + transition.
    let pool_info = vk::CommandPoolCreateInfo::default()
        .flags(vk::CommandPoolCreateFlags::TRANSIENT)
        .queue_family_index(queue_family);
    let pool = device
        .create_command_pool(&pool_info, None)
        .map_err(|error| format!("create_command_pool transient: {error}"))?;
    let cb_info = vk::CommandBufferAllocateInfo::default()
        .command_pool(pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(1);
    let cb = device
        .allocate_command_buffers(&cb_info)
        .map_err(|error| format!("allocate_command_buffers transient: {error}"))?[0];

    let begin = vk::CommandBufferBeginInfo::default()
        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    device
        .begin_command_buffer(cb, &begin)
        .map_err(|error| format!("begin_command_buffer transient: {error}"))?;

    // UNDEFINED -> TRANSFER_DST
    let to_dst = vk::ImageMemoryBarrier::default()
        .old_layout(vk::ImageLayout::UNDEFINED)
        .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
        .src_access_mask(vk::AccessFlags::empty())
        .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
        .image(image)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        });
    device.cmd_pipeline_barrier(
        cb,
        vk::PipelineStageFlags::TOP_OF_PIPE,
        vk::PipelineStageFlags::TRANSFER,
        vk::DependencyFlags::empty(),
        &[],
        &[],
        &[to_dst],
    );

    let region = vk::BufferImageCopy::default()
        .buffer_offset(0)
        .buffer_row_length(0)
        .buffer_image_height(0)
        .image_subresource(vk::ImageSubresourceLayers {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            mip_level: 0,
            base_array_layer: 0,
            layer_count: 1,
        })
        .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
        .image_extent(vk::Extent3D {
            width: atlas.width,
            height: atlas.height,
            depth: 1,
        });
    device.cmd_copy_buffer_to_image(
        cb,
        staging_buffer,
        image,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        &[region],
    );

    // TRANSFER_DST -> SHADER_READ_ONLY
    let to_shader = vk::ImageMemoryBarrier::default()
        .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
        .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
        .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
        .dst_access_mask(vk::AccessFlags::SHADER_READ)
        .image(image)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        });
    device.cmd_pipeline_barrier(
        cb,
        vk::PipelineStageFlags::TRANSFER,
        vk::PipelineStageFlags::FRAGMENT_SHADER,
        vk::DependencyFlags::empty(),
        &[],
        &[],
        &[to_shader],
    );

    device
        .end_command_buffer(cb)
        .map_err(|error| format!("end_command_buffer transient: {error}"))?;

    let cbs = [cb];
    let submit = vk::SubmitInfo::default().command_buffers(&cbs);
    device
        .queue_submit(queue, &[submit], vk::Fence::null())
        .map_err(|error| format!("queue_submit transient: {error}"))?;
    device
        .queue_wait_idle(queue)
        .map_err(|error| format!("queue_wait_idle transient: {error}"))?;

    device.destroy_command_pool(pool, None);
    device.destroy_buffer(staging_buffer, None);
    device.free_memory(staging_memory, None);

    // 4. View + sampler.
    let view_info = vk::ImageViewCreateInfo::default()
        .image(image)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(ATLAS_FORMAT)
        .components(vk::ComponentMapping::default())
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        });
    let view = device
        .create_image_view(&view_info, None)
        .map_err(|error| format!("create_image_view atlas: {error}"))?;

    let sampler_info = vk::SamplerCreateInfo::default()
        .mag_filter(vk::Filter::LINEAR)
        .min_filter(vk::Filter::LINEAR)
        .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
        .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
        .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
        .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
        .anisotropy_enable(false)
        .border_color(vk::BorderColor::FLOAT_TRANSPARENT_BLACK)
        .unnormalized_coordinates(false)
        .compare_enable(false)
        .min_lod(0.0)
        .max_lod(0.0);
    let sampler = device
        .create_sampler(&sampler_info, None)
        .map_err(|error| format!("create_sampler atlas: {error}"))?;

    Ok((image, memory, view, sampler))
}

unsafe fn create_buffer(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    device: &ash::Device,
    size: vk::DeviceSize,
    usage: vk::BufferUsageFlags,
    properties: vk::MemoryPropertyFlags,
) -> Result<(vk::Buffer, vk::DeviceMemory), String> {
    let info = vk::BufferCreateInfo::default()
        .size(size)
        .usage(usage)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);
    let buffer = device
        .create_buffer(&info, None)
        .map_err(|error| format!("create_buffer: {error}"))?;
    let req = device.get_buffer_memory_requirements(buffer);
    let mem_type =
        find_memory_type(instance, physical_device, req.memory_type_bits, properties)?;
    let alloc = vk::MemoryAllocateInfo::default()
        .allocation_size(req.size)
        .memory_type_index(mem_type);
    let memory = device
        .allocate_memory(&alloc, None)
        .map_err(|error| format!("allocate_memory buffer: {error}"))?;
    device
        .bind_buffer_memory(buffer, memory, 0)
        .map_err(|error| format!("bind_buffer_memory: {error}"))?;
    Ok((buffer, memory))
}

unsafe fn find_memory_type(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    type_filter: u32,
    properties: vk::MemoryPropertyFlags,
) -> Result<u32, String> {
    let mem_props = instance.get_physical_device_memory_properties(physical_device);
    for i in 0..mem_props.memory_type_count {
        if (type_filter & (1 << i)) != 0
            && mem_props.memory_types[i as usize]
                .property_flags
                .contains(properties)
        {
            return Ok(i);
        }
    }
    Err(format!(
        "no memory type matches filter {type_filter:b} props {properties:?}"
    ))
}

fn push_quad(verts: &mut Vec<UiVertex>, command: &DrawCommand) {
    let rect_min = [command.rect.x, command.rect.y];
    let rect_max = [
        command.rect.x + command.rect.width,
        command.rect.y + command.rect.height,
    ];
    let color = command.color.to_array();
    let radius = command.radius;
    let kind: u32 = match command.kind {
        DrawKind::Solid => 0,
        DrawKind::Glyph => 1,
    };

    let (qx0, qy0, qx1, qy1) = match command.kind {
        // Pad solid quads so SDF feathering has room outside the original rect.
        DrawKind::Solid => (
            rect_min[0] - AA_PAD,
            rect_min[1] - AA_PAD,
            rect_max[0] + AA_PAD,
            rect_max[1] + AA_PAD,
        ),
        DrawKind::Glyph => (rect_min[0], rect_min[1], rect_max[0], rect_max[1]),
    };

    // Glyph corner UVs span uv_min..uv_max; solid corners use 0,0 (unused).
    let (u0, v0, u1, v1) = (
        command.uv_min[0],
        command.uv_min[1],
        command.uv_max[0],
        command.uv_max[1],
    );

    let make = |pos: [f32; 2], uv: [f32; 2]| UiVertex {
        pos,
        color,
        rect_min,
        rect_max,
        radius,
        kind,
        uv,
    };

    // Two triangles per quad. Order: TL, TR, BR | TL, BR, BL.
    verts.push(make([qx0, qy0], [u0, v0]));
    verts.push(make([qx1, qy0], [u1, v0]));
    verts.push(make([qx1, qy1], [u1, v1]));

    verts.push(make([qx0, qy0], [u0, v0]));
    verts.push(make([qx1, qy1], [u1, v1]));
    verts.push(make([qx0, qy1], [u0, v1]));
}

// Avoid an unused import warning on `CStr` if Vulkan changes — keep one
// trivial use here so the import stays explicit.
const _: &CStr = unsafe { CStr::from_bytes_with_nul_unchecked(b"echover\0") };
