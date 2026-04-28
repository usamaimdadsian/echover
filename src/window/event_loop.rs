use std::num::NonZeroU32;

use glutin::config::{Config, ConfigTemplateBuilder, GlConfig};
use glutin::context::{ContextApi, ContextAttributesBuilder, PossiblyCurrentContext, Version};
use glutin::display::GetGlDisplay;
use glutin::prelude::*;
use glutin::surface::{GlSurface, Surface, SurfaceAttributesBuilder, SwapInterval, WindowSurface};
use glutin_winit::DisplayBuilder;
use raw_window_handle::HasWindowHandle;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

use crate::ui::state::AppState;
use crate::window::renderer::Renderer;

pub fn run() -> Result<(), String> {
    let event_loop =
        EventLoop::new().map_err(|error| format!("failed to create event loop: {error}"))?;
    let mut app = EchoverApp::default();
    event_loop
        .run_app(&mut app)
        .map_err(|error| format!("event loop terminated with error: {error}"))
}

#[derive(Default)]
struct EchoverApp {
    window_state: Option<WindowState>,
}

impl ApplicationHandler for EchoverApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window_state.is_some() {
            return;
        }

        match WindowState::new(event_loop) {
            Ok(mut state) => {
                state.request_redraw("ui state change");
                self.window_state = Some(state);
            }
            Err(error) => {
                tracing::error!("{error}");
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = self.window_state.as_mut() else {
            return;
        };

        if state.window.id() != window_id {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                state.resize(size);
                state.request_redraw("window resize");
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                let size = state.window.inner_size();
                state.resize(size);
                state.request_redraw("window resize");
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = state.render() {
                    tracing::error!("{error}");
                    event_loop.exit();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                state.app_state.mouse_position = (position.x as f32, position.y as f32);
                state.app_state.interaction.hovered_action =
                    state.renderer.action_at(state.app_state.mouse_position);
                state.request_redraw("mouse move");
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                state.app_state.interaction.mouse_down = true;
                let mut redraw_reason = "mouse click";

                if let Some(action) = state.renderer.handle_click(state.app_state.mouse_position) {
                    tracing::info!(?action, "ui action triggered");
                    if state.app_state.apply_action(action) {
                        redraw_reason = "ui state change";
                    }
                }

                state.request_redraw(redraw_reason);
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                state.app_state.interaction.mouse_down = false;
                state.request_redraw("mouse click");
            }
            WindowEvent::KeyboardInput { event, .. } if is_escape_pressed(&event) => {
                event_loop.exit()
            }
            _ => {}
        }
    }
}

struct WindowState {
    window: Window,
    gl_context: PossiblyCurrentContext,
    gl_surface: Surface<WindowSurface>,
    renderer: Renderer,
    app_state: AppState,
    pending_redraw_reason: Option<&'static str>,
    render_count: u64,
}

impl WindowState {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self, String> {
        let window_attributes = WindowAttributes::default()
            .with_title("Echover")
            .with_inner_size(PhysicalSize::new(1200, 800))
            .with_resizable(true);

        let config_template = ConfigTemplateBuilder::new().with_alpha_size(8);
        let display_builder = DisplayBuilder::new().with_window_attributes(Some(window_attributes));

        let (window, gl_config) = display_builder
            .build(event_loop, config_template, choose_gl_config)
            .map_err(|error| format!("failed to create window/display: {error}"))?;

        let window = window.ok_or_else(|| "glutin returned no window".to_owned())?;
        let raw_window_handle = window
            .window_handle()
            .map_err(|error| format!("failed to fetch raw window handle: {error}"))?
            .as_raw();

        let context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
            .build(Some(raw_window_handle));
        let fallback_context_attributes = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::Gles(None))
            .build(Some(raw_window_handle));

        let gl_display = gl_config.display();

        let not_current_context = unsafe {
            gl_display
                .create_context(&gl_config, &context_attributes)
                .or_else(|_| gl_display.create_context(&gl_config, &fallback_context_attributes))
        }
        .map_err(|error| format!("failed to create OpenGL context: {error}"))?;

        let size = window.inner_size();
        let gl_surface_attributes = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            raw_window_handle,
            non_zero(size.width),
            non_zero(size.height),
        );

        let gl_surface = unsafe {
            gl_config
                .display()
                .create_window_surface(&gl_config, &gl_surface_attributes)
        }
        .map_err(|error| format!("failed to create window surface: {error}"))?;

        let gl_context = not_current_context
            .make_current(&gl_surface)
            .map_err(|error| format!("failed to activate OpenGL context: {error}"))?;

        if let Err(error) = gl_surface.set_swap_interval(
            &gl_context,
            SwapInterval::Wait(NonZeroU32::new(1).expect("1 is non-zero")),
        ) {
            tracing::warn!("failed to set vsync: {error}");
        }

        let renderer = Renderer::new(
            &gl_config,
            size.width.max(1),
            size.height.max(1),
            window.scale_factor() as f32,
        )?;

        Ok(Self {
            window,
            gl_context,
            gl_surface,
            renderer,
            app_state: AppState::default(),
            pending_redraw_reason: None,
            render_count: 0,
        })
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }

        self.gl_surface.resize(
            &self.gl_context,
            non_zero(size.width),
            non_zero(size.height),
        );
        self.renderer
            .resize(size.width, size.height, self.window.scale_factor() as f32);
    }

    fn render(&mut self) -> Result<(), String> {
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }

        self.render_count = self.render_count.wrapping_add(1);
        tracing::info!(
            render_count = self.render_count,
            reason = self.pending_redraw_reason.take().unwrap_or("window/system"),
            "render"
        );

        self.renderer
            .render(&self.app_state, size.width, size.height)?;
        self.window.pre_present_notify();
        self.gl_surface
            .swap_buffers(&self.gl_context)
            .map_err(|error| format!("failed to swap buffers: {error}"))
    }

    fn request_redraw(&mut self, reason: &'static str) {
        self.pending_redraw_reason = Some(reason);
        self.window.request_redraw();
    }
}

fn choose_gl_config(configs: Box<dyn Iterator<Item = Config> + '_>) -> Config {
    configs
        .max_by_key(|config| config.num_samples())
        .expect("at least one OpenGL config should be available")
}

fn non_zero(value: u32) -> NonZeroU32 {
    NonZeroU32::new(value.max(1)).expect("value is forced to be non-zero")
}

fn is_escape_pressed(event: &KeyEvent) -> bool {
    event.state == ElementState::Pressed
        && matches!(event.logical_key, Key::Named(NamedKey::Escape))
}
