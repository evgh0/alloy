use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    HtmlCanvasElement, WebGl2RenderingContext as GL, WebGlProgram, WebGlShader,
};

mod camera;
mod mesh_transform;

const VERTEX_SHADER: &str = include_str!("shaders/triangle.vert.glsl");
const FRAGMENT_SHADER: &str = include_str!("shaders/triangle.frag.glsl");

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

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
    let mesh_transform = mesh_transform::MeshTransformUniform::new(&gl, &program).unwrap();

    fn push_instance_positions(positions: &mut Vec<f32>) {
        let spacing = 1.4_f32;
        for z in -2..=2 {
            for y in -2..=2 {
                for x in -2..=2 {
                    positions.extend_from_slice(&[
                        x as f32 * spacing,
                        y as f32 * spacing,
                        z as f32 * spacing,
                    ]);
                }
            }
        }
    }

    fn push_vertex(vertices: &mut Vec<f32>, position: [f32; 3], normal: [f32; 3]) {
        vertices.extend_from_slice(&[position[0], position[1], position[2], normal[0], normal[1], normal[2]]);
    }

    fn push_face(vertices: &mut Vec<f32>, a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3], normal: [f32; 3]) {
        push_vertex(vertices, a, normal);
        push_vertex(vertices, b, normal);
        push_vertex(vertices, c, normal);
        push_vertex(vertices, a, normal);
        push_vertex(vertices, c, normal);
        push_vertex(vertices, d, normal);
    }

    fn push_cube(vertices: &mut Vec<f32>) {
        let p000 = [-0.5, -0.5, -0.5];
        let p001 = [-0.5, -0.5, 0.5];
        let p010 = [-0.5, 0.5, -0.5];
        let p011 = [-0.5, 0.5, 0.5];
        let p100 = [0.5, -0.5, -0.5];
        let p101 = [0.5, -0.5, 0.5];
        let p110 = [0.5, 0.5, -0.5];
        let p111 = [0.5, 0.5, 0.5];

        push_face(vertices, p001, p101, p111, p011, [0.0, 0.0, 1.0]);
        push_face(vertices, p100, p110, p111, p101, [1.0, 0.0, 0.0]);
        push_face(vertices, p000, p001, p011, p010, [-1.0, 0.0, 0.0]);
        push_face(vertices, p010, p011, p111, p110, [0.0, 1.0, 0.0]);
        push_face(vertices, p000, p100, p101, p001, [0.0, -1.0, 0.0]);
        push_face(vertices, p000, p010, p110, p100, [0.0, 0.0, -1.0]);
    }

    let mut vertices: Vec<f32> = Vec::with_capacity(36 * 6);
    push_cube(&mut vertices);

    let mut instance_positions: Vec<f32> = Vec::with_capacity(5 * 5 * 5 * 3);
    push_instance_positions(&mut instance_positions);
    let instance_count = (instance_positions.len() / 3) as i32;

    let vao = gl.create_vertex_array().unwrap();
    gl.bind_vertex_array(Some(&vao));

    let buffer = gl.create_buffer().unwrap();
    gl.bind_buffer(GL::ARRAY_BUFFER, Some(&buffer));

    let vert_array = js_sys::Float32Array::from(vertices.as_slice());
    gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &vert_array, GL::STATIC_DRAW);

    let position = gl.get_attrib_location(&program, "a_position") as u32;
    gl.enable_vertex_attrib_array(position);
    gl.vertex_attrib_pointer_with_i32(position, 3, GL::FLOAT, false, 24, 0);

    let normal = gl.get_attrib_location(&program, "a_normal") as u32;
    gl.enable_vertex_attrib_array(normal);
    gl.vertex_attrib_pointer_with_i32(normal, 3, GL::FLOAT, false, 24, 12);

    let instance_buffer = gl.create_buffer().unwrap();
    gl.bind_buffer(GL::ARRAY_BUFFER, Some(&instance_buffer));

    let instance_array = js_sys::Float32Array::from(instance_positions.as_slice());
    gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &instance_array, GL::STATIC_DRAW);

    let instance_position = gl.get_attrib_location(&program, "a_instance_position") as u32;
    gl.enable_vertex_attrib_array(instance_position);
    gl.vertex_attrib_pointer_with_i32(instance_position, 3, GL::FLOAT, false, 12, 0);
    gl.vertex_attrib_divisor(instance_position, 1);

    gl.enable(GL::DEPTH_TEST);
    gl.depth_func(GL::LESS);
    gl.enable(GL::CULL_FACE);
    gl.cull_face(GL::BACK);

    let gl = Rc::new(gl);
    let canvas = Rc::new(canvas);
    let window = Rc::new(window);
    let camera = Rc::new(camera);
    let mesh_transform = Rc::new(mesh_transform);

    let raf_handle: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let raf_handle_inner = Rc::clone(&raf_handle);
    let gl_inner = Rc::clone(&gl);
    let canvas_inner = Rc::clone(&canvas);
    let window_inner = Rc::clone(&window);
    let camera_inner = Rc::clone(&camera);
    let mesh_transform_inner = Rc::clone(&mesh_transform);

    *raf_handle_inner.borrow_mut() = Some(Closure::wrap(Box::new(move |time: f64| {
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

        camera_inner.update_aspect(gl_inner.as_ref(), logical_width as f32 / logical_height as f32);

        let angle = (time as f32) * 0.001;
        let half_angle = angle * 0.5;
        let rotation = [0.0, half_angle.sin(), 0.0, half_angle.cos()];

        mesh_transform_inner.update_rotation(gl_inner.as_ref(), rotation);

        gl_inner.viewport(0, 0, width as i32, height as i32);
        gl_inner.clear_color(0.02, 0.02, 0.05, 1.0);
        gl_inner.clear(GL::COLOR_BUFFER_BIT | GL::DEPTH_BUFFER_BIT);
        gl_inner.draw_arrays_instanced(GL::TRIANGLES, 0, 36, instance_count);

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
