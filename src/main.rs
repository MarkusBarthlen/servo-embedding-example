extern crate glutin;
extern crate servo;

use servo::gl;
//use glutin::GlContext;
use servo::BrowserId;
use servo::embedder_traits::EventLoopWaker;
use servo::compositing::windowing::{WindowEvent, WindowMethods, EmbedderMethods};
use servo::euclid::{Point2D, Size2D, Scale,
                    Vector2D};
use servo::ipc_channel::ipc;

use webvr_traits::WebVRMainThreadHeartbeat;
use servo::script_traits::{LoadData, TouchEventType};
use servo::servo_config::opts;
//use servo::servo_config::resource_files::set_resources_path;
use servo::servo_geometry::DeviceIndependentPixel;
use servo::servo_url::ServoUrl;
use servo::style_traits::DevicePixel;
use webvr::VRServiceManager;
use std::env;
use std::rc::Rc;
use std::sync::Arc;
use servo_media::player::context::{GlApi, GlContext, NativeDisplay};


pub struct GlutinEventLoopWaker {
    proxy: Arc<glutin::EventsLoopProxy>,
}

impl EventLoopWaker for GlutinEventLoopWaker {
    // Use by servo to share the "event loop waker" across threads
    fn clone(&self) -> Box<EventLoopWaker + Send> {
        Box::new(GlutinEventLoopWaker {
            proxy: self.proxy.clone(),
        })
    }
    // Called by servo when the main thread needs to wake up
    fn wake(&self) {
        self.proxy.wakeup().expect("wakeup eventloop failed");
    }
}

struct Window {
    glutin_window: glutin::GlWindow,
    waker: Box<EventLoopWaker>,
    gl: Rc<gl::Gl>,
}

fn main() {
    println!("Servo version: {}", servo::config::servo_version());

    let mut event_loop = glutin::EventsLoop::new();

    let builder = glutin::WindowBuilder::new().with_dimensions(800, 600);
    let gl_version = glutin::GlRequest::Specific(glutin::Api::OpenGl, (3, 2));
    let context = glutin::ContextBuilder::new()
        .with_gl(gl_version)
        .with_vsync(true);
    let window = glutin::GlWindow::new(builder, context, &event_loop).unwrap();

    window.show();

    let gl = unsafe {
        window
            .context()
            .make_current()
            .expect("Couldn't make window current");
        gl::GlFns::load_with(|s| window.context().get_proc_address(s) as *const _)
    };

    let event_loop_waker = Box::new(GlutinEventLoopWaker {
        proxy: Arc::new(event_loop.create_proxy()),
    });

    let path = env::current_dir().unwrap().join("resources");
    let path = path.to_str().unwrap().to_string();
    //set_resources_path(Some(path));
    //opts::set_defaults(opts::default_opts());

    let window = Rc::new(Window {
        glutin_window: window,
        waker: event_loop_waker,
        gl: gl,
    });

    let mut servo = servo::Servo::new(window.clone());

    let url = ServoUrl::parse("https://servo.org").unwrap();
    let (sender, receiver) = ipc::channel().unwrap();
    servo.handle_events(vec![WindowEvent::NewBrowser(url, sender)]);
    let browser_id = receiver.recv().unwrap();
    servo.handle_events(vec![WindowEvent::SelectBrowser(browser_id)]);

    let mut pointer = (0.0, 0.0);

    event_loop.run_forever(|event| {
        // Blocked until user event or until servo unblocks it
        match event {
            // This is the event triggered by GlutinEventLoopWaker
            glutin::Event::Awakened => {
                servo.handle_events(vec![]);
            }

            // Mousemove
            glutin::Event::WindowEvent {
                event:
                    glutin::WindowEvent::CursorMoved {
                        position: (x, y), ..
                    },
                ..
            } => {
                pointer = (x, y);
                let event =
                    WindowEvent::MouseWindowMoveEventClass(DevicePoint::new(x as f32, y as f32));
                servo.handle_events(vec![event]);
            }

            // reload when R is pressed
            glutin::Event::WindowEvent {
                event:
                    glutin::WindowEvent::KeyboardInput {
                        input:
                            glutin::KeyboardInput {
                                state: glutin::ElementState::Pressed,
                                virtual_keycode: Some(glutin::VirtualKeyCode::R),
                                ..
                            },
                        ..
                    },
                ..
            } => {
                let event = WindowEvent::Reload(browser_id);
                servo.handle_events(vec![event]);
            }

            // Scrolling
            glutin::Event::WindowEvent {
                event: glutin::WindowEvent::MouseWheel { delta, phase, .. },
                ..
            } => {
                let pointer = DeviceIntPoint::new(pointer.0 as i32, pointer.1 as i32);
                let (dx, dy) = match delta {
                    glutin::MouseScrollDelta::LineDelta(dx, dy) => {
                        (dx, dy * 38.0 /*line height*/)
                    }
                    glutin::MouseScrollDelta::PixelDelta(dx, dy) => (dx, dy),
                };
                let scroll_location =
                    servo::webrender_api::ScrollLocation::Delta(Vector2D::new(dx, dy));
                let phase = match phase {
                    glutin::TouchPhase::Started => TouchEventType::Down,
                    glutin::TouchPhase::Moved => TouchEventType::Move,
                    glutin::TouchPhase::Ended => TouchEventType::Up,
                    glutin::TouchPhase::Cancelled => TouchEventType::Up,
                };
                let event = WindowEvent::Scroll(scroll_location, pointer, phase);
                servo.handle_events(vec![event]);
            }
            glutin::Event::WindowEvent {
                event: glutin::WindowEvent::Resized(width, height),
                ..
            } => {
                let event = WindowEvent::Resize;
                servo.handle_events(vec![event]);
                window.glutin_window.resize(width, height);
            }
            _ => {}
        }
        glutin::ControlFlow::Continue
    });
}

impl WindowMethods for Window {
    fn prepare_for_composite(&self, _width: usize, _height: usize) -> bool {
        true
    }

    fn present(&self) {
        self.glutin_window.swap_buffers().unwrap();
    }

    fn gl(&self) -> Rc<gl::Gl> {
        self.gl.clone()
    }

    fn set_animation_state(&self, _state: AnimationState) {

    }

    fn get_gl_context(&self) -> GlContext {
        let gl_context = {
                    use glutin::os::unix::RawHandle;

                    match raw_handle {
                        RawHandle::Egl(egl_context) => GlContext::Egl(egl_context as usize),
                        RawHandle::Glx(glx_context) => GlContext::Glx(glx_context as usize),
                    }
                };
    }

    fn get_native_display(&self) -> NativeDisplay{
        let native_display = if let Some(display) =
                    unsafe { windowed_context.context().get_egl_display() }
                {
                    NativeDisplay::Egl(display as usize)
                } else {
                    use glutin::os::unix::WindowExt;

                    if let Some(display) = windowed_context.window().get_wayland_display() {
                        NativeDisplay::Wayland(display as usize)
                    } else if let Some(display) = windowed_context.window().get_xlib_display() {
                        NativeDisplay::X11(display as usize)
                    } else {
                        NativeDisplay::Unknown
                    }
                };
        native_display?        
    }
    
    fn get_gl_api(&self) -> GlApi{
        GlApi::OpenGL3
    }

 //   fn hidpi_factor(&self) -> Scale<f32, DeviceIndependentPixel, DevicePixel> {
 //       Scale::new(self.glutin_window.hidpi_factor())
 //   }

 //   fn screen_size(&self, _id: BrowserId) -> Size2D<u32> {
 //       let monitor = self.glutin_window.get_current_monitor();
 //       let (monitor_width, monitor_height) = monitor.get_dimensions();
 //       Size2D::new(monitor_width, monitor_height)
 //   }
}

impl EmbedderMethods for Window {

    fn create_event_loop_waker(&self) -> Box<EventLoopWaker> {
        self.waker.clone()
    }

    fn register_vr_services(
        &mut self,
        _: &mut VRServiceManager,
        _: &mut Vec<Box<dyn WebVRMainThreadHeartbeat>>,
    ) {
    }

        /// Register services with a WebXR Registry.
    fn register_webxr(&mut self, _: &mut webxr_api::MainThreadRegistry) {}
}
