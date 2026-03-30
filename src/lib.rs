use std::cell::RefCell;
use std::cell::Cell;
use std::collections::HashSet;
use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    console, HtmlCanvasElement, WebGl2RenderingContext as GL, WebGlBuffer, WebGlProgram,
    WebGlShader, WebGlVertexArrayObject,
};

mod camera;
mod primitive;
mod batcher;
mod example;
use batcher::Batcher;

const VERTEX_SHADER: &str = include_str!("shaders/triangle.vert.glsl");
const FRAGMENT_SHADER: &str = include_str!("shaders/triangle.frag.glsl");

pub struct Phong;

pub trait Scene {
    fn update(&mut self, time: f32);
    fn draw(&self, batcher: &mut Batcher);
}

pub fn canvas() -> Result<CanvasApp, JsValue> {
    CanvasApp::init()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CanvasModifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct CanvasMouseButtonEvent {
    pub button: u16,
    pub x: f64,
    pub y: f64,
    pub modifiers: CanvasModifiers,
}

#[derive(Clone, Copy, Debug)]
pub struct CanvasMouseMoveEvent {
    pub x: f64,
    pub y: f64,
    pub dx: f64,
    pub dy: f64,
    pub buttons: u16,
    pub modifiers: CanvasModifiers,
}

#[derive(Clone, Copy, Debug)]
pub struct CanvasWheelEvent {
    pub delta_x: f64,
    pub delta_y: f64,
    pub delta_z: f64,
    pub x: f64,
    pub y: f64,
    pub modifiers: CanvasModifiers,
}

#[derive(Default)]
struct CanvasListeners {
    mouse_down: Option<Box<dyn FnMut(CanvasMouseButtonEvent)>>,
    mouse_move: Option<Box<dyn FnMut(CanvasMouseMoveEvent)>>,
    mouse_up: Option<Box<dyn FnMut(CanvasMouseButtonEvent)>>,
    wheel: Option<Box<dyn FnMut(CanvasWheelEvent)>>,
}

pub struct CanvasApp {
    gl: Rc<GL>,
    canvas: Rc<HtmlCanvasElement>,
    window: Rc<web_sys::Window>,
    camera: Rc<RefCell<camera::CameraUniform>>,
    batcher: Batcher,
    update_frequency: u32,
    event_logging: bool,
    start_in_freeflight: bool,
    listeners: CanvasListeners,
    scene: Option<Box<dyn Scene>>,
}

#[derive(Default)]
struct InputState {
    mouse_dx: f64,
    mouse_dy: f64,
    wheel_delta: f64,
    mouse_buttons: u16,
    modifiers: CanvasModifiers,
    keys_down: HashSet<String>,
    toggle_freeflight_requested: bool,
}

impl InputState {
    fn take_frame(&mut self) -> InputFrame {
        let frame = InputFrame {
            mouse_dx: self.mouse_dx,
            mouse_dy: self.mouse_dy,
            wheel_delta: self.wheel_delta,
            mouse_buttons: self.mouse_buttons,
            modifiers: self.modifiers,
            keys_down: self.keys_down.clone(),
            toggle_freeflight_requested: self.toggle_freeflight_requested,
        };

        self.mouse_dx = 0.0;
        self.mouse_dy = 0.0;
        self.wheel_delta = 0.0;
        self.toggle_freeflight_requested = false;

        frame
    }
}

struct InputFrame {
    mouse_dx: f64,
    mouse_dy: f64,
    wheel_delta: f64,
    mouse_buttons: u16,
    modifiers: CanvasModifiers,
    keys_down: HashSet<String>,
    toggle_freeflight_requested: bool,
}

struct MeshBatch {
    _vao: WebGlVertexArrayObject,
    _vertex_buffer: WebGlBuffer,
    _instance_buffer: WebGlBuffer,
    vertex_count: i32,
    instance_count: Cell<i32>,
    instance_data: RefCell<Vec<f32>>,
}

impl MeshBatch {
    fn new(
        gl: &GL,
        program: &WebGlProgram,
        vertices: &[f32],
        instance_data: &[f32],
    ) -> Result<Self, JsValue> {
        let vao = gl
            .create_vertex_array()
            .ok_or_else(|| JsValue::from_str("failed to create vertex array"))?;
        gl.bind_vertex_array(Some(&vao));

        let vertex_buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("failed to create mesh vertex buffer"))?;
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&vertex_buffer));

        let vertex_array = js_sys::Float32Array::from(vertices);
        gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &vertex_array, GL::STATIC_DRAW);

        let position = gl.get_attrib_location(program, "a_position") as u32;
        gl.enable_vertex_attrib_array(position);
        gl.vertex_attrib_pointer_with_i32(position, 3, GL::FLOAT, false, 24, 0);

        let normal = gl.get_attrib_location(program, "a_normal") as u32;
        gl.enable_vertex_attrib_array(normal);
        gl.vertex_attrib_pointer_with_i32(normal, 3, GL::FLOAT, false, 24, 12);

        let instance_buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("failed to create instance buffer"))?;
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&instance_buffer));

        let instance_array = js_sys::Float32Array::from(instance_data);
        gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &instance_array, GL::DYNAMIC_DRAW);

        let instance_position = gl.get_attrib_location(program, "a_instance_position") as u32;
        gl.enable_vertex_attrib_array(instance_position);
        gl.vertex_attrib_pointer_with_i32(instance_position, 3, GL::FLOAT, false, 32, 0);
        gl.vertex_attrib_divisor(instance_position, 1);

        let instance_rotation = gl.get_attrib_location(program, "a_instance_rotation") as u32;
        gl.enable_vertex_attrib_array(instance_rotation);
        gl.vertex_attrib_pointer_with_i32(instance_rotation, 4, GL::FLOAT, false, 32, 12);
        gl.vertex_attrib_divisor(instance_rotation, 1);

        let instance_mesh_index = gl.get_attrib_location(program, "a_instance_mesh_index") as u32;
        gl.enable_vertex_attrib_array(instance_mesh_index);
        gl.vertex_attrib_pointer_with_i32(instance_mesh_index, 1, GL::FLOAT, false, 32, 28);
        gl.vertex_attrib_divisor(instance_mesh_index, 1);

        Ok(Self {
            _vao: vao,
            _vertex_buffer: vertex_buffer,
            _instance_buffer: instance_buffer,
            vertex_count: (vertices.len() / 6) as i32,
            instance_count: Cell::new((instance_data.len() / 8) as i32),
            instance_data: RefCell::new(instance_data.to_vec()),
        })
    }

    fn upload_instances(&self, gl: &GL, instance_data: &[f32]) {
        *self.instance_data.borrow_mut() = instance_data.to_vec();

        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&self._instance_buffer));

        let instance_array = js_sys::Float32Array::from(instance_data);
        gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &instance_array, GL::DYNAMIC_DRAW);
        self.instance_count.set((instance_data.len() / 8) as i32);
    }
}

impl CanvasApp {
    pub fn init() -> Result<Self, JsValue> {
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let canvas = document
            .get_element_by_id("canvas")
            .unwrap()
            .dyn_into::<HtmlCanvasElement>()
            .unwrap();

        let gl = canvas
            .get_context("webgl2")?
            .unwrap()
            .dyn_into::<GL>()
            .unwrap();

        let vert_shader = compile_shader(&gl, GL::VERTEX_SHADER, VERTEX_SHADER).unwrap();
        let frag_shader = compile_shader(&gl, GL::FRAGMENT_SHADER, FRAGMENT_SHADER).unwrap();

        let program = link_program(&gl, &vert_shader, &frag_shader).unwrap();
        gl.use_program(Some(&program));
        let camera = camera::CameraUniform::new(&gl, &program).unwrap();
        let batcher = Batcher::new(&gl, &program);

        gl.enable(GL::DEPTH_TEST);
        gl.depth_func(GL::LESS);
        gl.enable(GL::CULL_FACE);
        gl.cull_face(GL::BACK);

        Ok(Self {
            gl: Rc::new(gl),
            canvas: Rc::new(canvas),
            window: Rc::new(window),
            camera: Rc::new(RefCell::new(camera)),
            batcher,
            update_frequency: 60,
            event_logging: false,
            start_in_freeflight: false,
            listeners: CanvasListeners::default(),
            scene: None,
        })
    }

    pub fn shading(self, _shading: Phong) -> Self {
        self
    }

    pub fn camera(self) -> Self {
        self
    }

    pub fn freeflight(mut self) -> Self {
        self.start_in_freeflight = true;
        self
    }

    pub fn enable_logging(mut self) -> Self {
        self.event_logging = true;
        self
    }

    pub fn on_mouse_down<F>(mut self, callback: F) -> Self
    where
        F: 'static + FnMut(CanvasMouseButtonEvent),
    {
        self.listeners.mouse_down = Some(Box::new(callback));
        self
    }

    pub fn on_mouse_move<F>(mut self, callback: F) -> Self
    where
        F: 'static + FnMut(CanvasMouseMoveEvent),
    {
        self.listeners.mouse_move = Some(Box::new(callback));
        self
    }

    pub fn on_mouse_up<F>(mut self, callback: F) -> Self
    where
        F: 'static + FnMut(CanvasMouseButtonEvent),
    {
        self.listeners.mouse_up = Some(Box::new(callback));
        self
    }

    pub fn on_wheel<F>(mut self, callback: F) -> Self
    where
        F: 'static + FnMut(CanvasWheelEvent),
    {
        self.listeners.wheel = Some(Box::new(callback));
        self
    }

    pub fn update_frequency(mut self, update_frequency: u32) -> Self {
        self.update_frequency = update_frequency.max(1);
        self
    }

    pub fn scene<S: Scene + 'static>(mut self, scene: S) -> Self {
        self.scene = Some(Box::new(scene));
        self
    }

    pub fn start(self) -> Result<(), JsValue> {
        let CanvasApp {
            gl,
            canvas,
            window,
            camera,
            batcher,
            update_frequency,
            event_logging,
            start_in_freeflight,
            listeners,
            scene,
        } = self;

        let scene = Rc::new(RefCell::new(scene.expect("scene must be set before start")));
        let input_state = Rc::new(RefCell::new(InputState::default()));
        let listener_handles = install_event_listeners(
            canvas.as_ref(),
            window.as_ref(),
            listeners,
            Rc::clone(&input_state),
            event_logging,
        )?;
        let raf_handle: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
        let raf_handle_inner = Rc::clone(&raf_handle);
        let gl_inner = Rc::clone(&gl);
        let canvas_inner = Rc::clone(&canvas);
        let window_inner = Rc::clone(&window);
        let camera_inner = Rc::clone(&camera);
        let mut batcher = batcher;
        let scene_inner = Rc::clone(&scene);
        let input_state_inner = Rc::clone(&input_state);
        let update_interval = 1000.0 / update_frequency as f64;
        let last_update_time = Rc::new(RefCell::new(f64::NEG_INFINITY));
        let last_update_time_inner = Rc::clone(&last_update_time);
        let last_frame_time = Rc::new(RefCell::new(f64::NEG_INFINITY));
        let last_frame_time_inner = Rc::clone(&last_frame_time);
        let last_draw_time = Rc::new(RefCell::new(f64::NEG_INFINITY));
        let last_draw_time_inner = Rc::clone(&last_draw_time);

        camera_inner.borrow_mut().set_freeflight(start_in_freeflight);

        *raf_handle_inner.borrow_mut() = Some(Closure::wrap(Box::new(move |time: f64| {
            let _keep_listener_handles_alive = &listener_handles;
            let dt_seconds = {
                let mut last_time = last_frame_time_inner.borrow_mut();
                let dt = if *last_time == f64::NEG_INFINITY {
                    0.0
                } else {
                    ((time - *last_time) / 1000.0) as f32
                };
                *last_time = time;
                dt
            };

            let frame_input = input_state_inner.borrow_mut().take_frame();
            let logical_width = window_inner
                .inner_width()
                .ok()
                .and_then(|value| value.as_f64())
                .unwrap_or(canvas_inner.width() as f64)
                .max(1.0) as u32;
            let logical_height = window_inner
                .inner_height()
                .ok()
                .and_then(|value| value.as_f64())
                .unwrap_or(canvas_inner.height() as f64)
                .max(1.0) as u32;
            let dpr = window_inner.device_pixel_ratio().max(1.0);
            let width = (logical_width as f64 * dpr).round().max(1.0) as u32;
            let height = (logical_height as f64 * dpr).round().max(1.0) as u32;

            if canvas_inner.width() != width || canvas_inner.height() != height {
                canvas_inner.set_width(width);
                canvas_inner.set_height(height);
            }

            {
                let mut camera = camera_inner.borrow_mut();

                if frame_input.toggle_freeflight_requested {
                    camera.toggle_freeflight();
                }

                if camera.is_freeflight() {
                    camera.orbit(frame_input.mouse_dx as f32, frame_input.mouse_dy as f32);

                    let forward = key_down(&frame_input.keys_down, "KeyW") as i32
                        - key_down(&frame_input.keys_down, "KeyS") as i32;
                    let strafe = key_down(&frame_input.keys_down, "KeyD") as i32
                        - key_down(&frame_input.keys_down, "KeyA") as i32;
                    let vertical = key_down(&frame_input.keys_down, "Space") as i32
                        - key_down(&frame_input.keys_down, "ShiftLeft") as i32
                        - key_down(&frame_input.keys_down, "ShiftRight") as i32;

                    camera.freeflight_move(forward as f32, strafe as f32, vertical as f32, dt_seconds);

                    if frame_input.wheel_delta.abs() > f64::EPSILON {
                        camera.dolly(-(frame_input.wheel_delta as f32) * 0.01);
                    }
                } else {
                    if frame_input.modifiers.ctrl && frame_input.mouse_buttons & 1 != 0 {
                        camera.orbit(-frame_input.mouse_dx as f32, -frame_input.mouse_dy as f32);
                    } else if frame_input.modifiers.ctrl && frame_input.mouse_buttons & 4 != 0 {
                        camera.pan(frame_input.mouse_dx as f32, frame_input.mouse_dy as f32);
                    } else if frame_input.modifiers.ctrl && frame_input.mouse_buttons & 2 != 0 {
                        camera.dolly((frame_input.mouse_dy as f32) * -0.01);
                    }

                    if frame_input.wheel_delta.abs() > f64::EPSILON {
                        camera.dolly((frame_input.wheel_delta as f32) * 0.01);
                    }
                }

                camera.update_aspect(gl_inner.as_ref(), logical_width as f32 / logical_height as f32);
            }

            if *last_update_time_inner.borrow() == f64::NEG_INFINITY
                || time - *last_update_time_inner.borrow() >= update_interval
            {
                *last_update_time_inner.borrow_mut() = time;
                scene_inner.borrow_mut().update(time as f32);
            }

            batcher.clear();
            scene_inner.borrow().draw(&mut batcher);
            batcher.flush();

            gl_inner.viewport(0, 0, width as i32, height as i32);
            gl_inner.clear_color(0.02, 0.02, 0.05, 1.0);
            gl_inner.clear(GL::COLOR_BUFFER_BIT | GL::DEPTH_BUFFER_BIT);

            batcher.draw();

            /*
            if *last_draw_time_inner.borrow() != f64::NEG_INFINITY {
                let delta_ms = time - *last_draw_time_inner.borrow();
                let estimated_fps = if delta_ms > 0.0 {
                    1000.0 / delta_ms
                } else {
                    f64::INFINITY
                };
                console::log_1(
                    &format!("frame dt: {:.2} ms (~{:.1} fps)", delta_ms, estimated_fps).into(),
                );
            }
             */

            *last_draw_time_inner.borrow_mut() = time;

            // *fps_frame_count_inner.borrow_mut() += 1;
            // if *fps_last_sample_time_inner.borrow() == f64::NEG_INFINITY {
            //     *fps_last_sample_time_inner.borrow_mut() = time;
            // } else {
            //     let elapsed = time - *fps_last_sample_time_inner.borrow();
            //     if elapsed >= 1000.0 {
            //         let frames = *fps_frame_count_inner.borrow();
            //         let fps = (frames as f64 * 1000.0 / elapsed).round() as u32;
            //         console::log_1(&format!("fps: {}", fps).into());
            //         *fps_frame_count_inner.borrow_mut() = 0;
            //         *fps_last_sample_time_inner.borrow_mut() = time;
            //     }
            // }

            window_inner
                .request_animation_frame(
                    raf_handle
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .as_ref()
                        .unchecked_ref(),
                )
                .unwrap();
        }) as Box<dyn FnMut(f64)>));

        window
            .request_animation_frame(
                raf_handle_inner
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .as_ref()
                    .unchecked_ref(),
            )
            .unwrap();

        Ok(())
    }
}

fn install_event_listeners(
    canvas: &HtmlCanvasElement,
    window: &web_sys::Window,
    mut listeners: CanvasListeners,
    input_state: Rc<RefCell<InputState>>,
    event_logging: bool,
) -> Result<Vec<Closure<dyn FnMut(web_sys::Event)>>, JsValue> {
    let mut handles: Vec<Closure<dyn FnMut(web_sys::Event)>> = Vec::new();

    let mouse_down_input_state = Rc::clone(&input_state);
    let mut mouse_down_callback = listeners.mouse_down.take();
    let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
        let event = event.dyn_into::<web_sys::MouseEvent>().unwrap();
        mouse_down_input_state.borrow_mut().mouse_buttons = event.buttons();
        let event = CanvasMouseButtonEvent {
            button: event.button() as u16,
            x: event.client_x() as f64,
            y: event.client_y() as f64,
            modifiers: CanvasModifiers {
                shift: event.shift_key(),
                ctrl: event.ctrl_key(),
                alt: event.alt_key(),
                meta: event.meta_key(),
            },
        };
        if event_logging {
            console::log_1(&format!("mouse_down: {:?}", event).into());
        }
        if let Some(callback) = mouse_down_callback.as_mut() {
            callback(event);
        }
    }) as Box<dyn FnMut(web_sys::Event)>);

    canvas.add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref())?;
    handles.push(closure);

    let mouse_move_input_state = Rc::clone(&input_state);
    let mut mouse_move_callback = listeners.mouse_move.take();
    let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
        let event = event.dyn_into::<web_sys::MouseEvent>().unwrap();
        {
            let mut input = mouse_move_input_state.borrow_mut();
            input.mouse_dx += event.movement_x() as f64;
            input.mouse_dy += event.movement_y() as f64;
            input.mouse_buttons = event.buttons();
            input.modifiers = CanvasModifiers {
                shift: event.shift_key(),
                ctrl: event.ctrl_key(),
                alt: event.alt_key(),
                meta: event.meta_key(),
            };
        }
        let event = CanvasMouseMoveEvent {
            x: event.client_x() as f64,
            y: event.client_y() as f64,
            dx: event.movement_x() as f64,
            dy: event.movement_y() as f64,
            buttons: event.buttons(),
            modifiers: CanvasModifiers {
                shift: event.shift_key(),
                ctrl: event.ctrl_key(),
                alt: event.alt_key(),
                meta: event.meta_key(),
            },
        };
        if event_logging {
            console::log_1(&format!("mouse_move: {:?}", event).into());
        }
        if let Some(callback) = mouse_move_callback.as_mut() {
            callback(event);
        }
    }) as Box<dyn FnMut(web_sys::Event)>);

    window.add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref())?;
    handles.push(closure);

    let mouse_up_input_state = Rc::clone(&input_state);
    let mut mouse_up_callback = listeners.mouse_up.take();
    let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
        let event = event.dyn_into::<web_sys::MouseEvent>().unwrap();
        mouse_up_input_state.borrow_mut().mouse_buttons = event.buttons();
        let event = CanvasMouseButtonEvent {
            button: event.button() as u16,
            x: event.client_x() as f64,
            y: event.client_y() as f64,
            modifiers: CanvasModifiers {
                shift: event.shift_key(),
                ctrl: event.ctrl_key(),
                alt: event.alt_key(),
                meta: event.meta_key(),
            },
        };
        if event_logging {
            console::log_1(&format!("mouse_up: {:?}", event).into());
        }
        if let Some(callback) = mouse_up_callback.as_mut() {
            callback(event);
        }
    }) as Box<dyn FnMut(web_sys::Event)>);

    window.add_event_listener_with_callback("mouseup", closure.as_ref().unchecked_ref())?;
    handles.push(closure);

    let mut wheel_callback = listeners.wheel.take();
    let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
        let event = event.dyn_into::<web_sys::WheelEvent>().unwrap();
        event.prevent_default();
        let event = CanvasWheelEvent {
            delta_x: event.delta_x(),
            delta_y: event.delta_y(),
            delta_z: event.delta_z(),
            x: event.client_x() as f64,
            y: event.client_y() as f64,
            modifiers: CanvasModifiers {
                shift: event.shift_key(),
                ctrl: event.ctrl_key(),
                alt: event.alt_key(),
                meta: event.meta_key(),
            },
        };
        if event_logging {
            console::log_1(&format!("wheel: {:?}", event).into());
        }
        if let Some(callback) = wheel_callback.as_mut() {
            callback(event);
        }
    }) as Box<dyn FnMut(web_sys::Event)>);

    canvas.add_event_listener_with_callback("wheel", closure.as_ref().unchecked_ref())?;
    handles.push(closure);

    {
        let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
            event.prevent_default();
        }) as Box<dyn FnMut(web_sys::Event)>);

        canvas.add_event_listener_with_callback("contextmenu", closure.as_ref().unchecked_ref())?;
        handles.push(closure);
    }

    {
        let input_state = Rc::clone(&input_state);
        let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
            let event = event.dyn_into::<web_sys::KeyboardEvent>().unwrap();
            let code = event.code();
            let mut input = input_state.borrow_mut();
            input.keys_down.insert(code.clone());

            if code == "KeyF" && !event.repeat() {
                input.toggle_freeflight_requested = true;
            }

            if is_freeflight_key(&code) {
                event.prevent_default();
            }

            if event_logging {
                console::log_1(&format!("keydown: {}", code).into());
            }
        }) as Box<dyn FnMut(web_sys::Event)>);

        window.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())?;
        handles.push(closure);
    }

    {
        let input_state = Rc::clone(&input_state);
        let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
            let event = event.dyn_into::<web_sys::KeyboardEvent>().unwrap();
            let code = event.code();
            input_state.borrow_mut().keys_down.remove(&code);

            if is_freeflight_key(&code) {
                event.prevent_default();
            }

            if event_logging {
                console::log_1(&format!("keyup: {}", code).into());
            }
        }) as Box<dyn FnMut(web_sys::Event)>);

        window.add_event_listener_with_callback("keyup", closure.as_ref().unchecked_ref())?;
        handles.push(closure);
    }

    Ok(handles)
}

fn key_down(keys_down: &HashSet<String>, code: &str) -> bool {
    keys_down.contains(code)
}

fn is_freeflight_key(code: &str) -> bool {
    matches!(
        code,
        "KeyW" | "KeyA" | "KeyS" | "KeyD" | "Space" | "ShiftLeft" | "ShiftRight" | "KeyF"
    )
}


fn compile_shader(gl: &GL, shader_type: u32, source: &str) -> Result<WebGlShader, String> {
    let shader = gl.create_shader(shader_type).ok_or("failed to create shader")?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);

    if gl
        .get_shader_parameter(&shader, GL::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        Err(gl
            .get_shader_info_log(&shader)
            .unwrap_or_else(|| "shader compilation failed".into()))
    }
}

fn link_program(gl: &GL, vert: &WebGlShader, frag: &WebGlShader) -> Result<WebGlProgram, String> {
    let program = gl.create_program().ok_or("failed to create program")?;
    gl.attach_shader(&program, vert);
    gl.attach_shader(&program, frag);
    gl.link_program(&program);

    if gl
        .get_program_parameter(&program, GL::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(program)
    } else {
        Err(gl
            .get_program_info_log(&program)
            .unwrap_or_else(|| "program link failed".into()))
    }
}
