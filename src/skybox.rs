use std::cell::Cell;
use std::f32::consts::PI;
use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{
    CanvasRenderingContext2d, HtmlCanvasElement, HtmlImageElement, WebGl2RenderingContext as GL,
    WebGlBuffer, WebGlProgram, WebGlTexture, WebGlVertexArrayObject,
    Response,
};

use crate::primitive;

use super::{compile_shader, link_program};

const SKYBOX_VERTEX_SHADER: &str = include_str!("shaders/skybox.vert.glsl");
const SKYBOX_FRAGMENT_SHADER: &str = include_str!("shaders/skybox.frag.glsl");

pub struct Skybox {
    source: SkyboxSource,
}

#[allow(dead_code)]
enum SkyboxSource {
    Cubemap { face_urls: [String; 6] },
    Hdr { url: String, face_size: u32 },
    Equirectangular { url: String, face_size: u32 },
}

impl Skybox {
    pub fn cubemap_from_urls<U: Into<String>>(face_urls: [U; 6]) -> Self {
        Self {
            source: SkyboxSource::Cubemap {
                face_urls: face_urls.map(Into::into),
            },
        }
    }

    pub fn hdri_from_url<U: Into<String>>(url: U) -> Self {
        Self::hdri_from_url_with_face_size(url, 1024)
    }

    pub fn hdri_from_url_with_face_size<U: Into<String>>(url: U, face_size: u32) -> Self {
        Self {
            source: SkyboxSource::Hdr {
                url: url.into(),
                face_size: face_size.max(1),
            },
        }
    }
}

pub(crate) struct SkyboxRenderer {
    gl: GL,
    program: WebGlProgram,
    vao: WebGlVertexArrayObject,
    _vertex_buffer: WebGlBuffer,
    texture: WebGlTexture,
    _images: Vec<HtmlImageElement>,
    _load_callbacks: Vec<Closure<dyn FnMut(web_sys::Event)>>,
}

impl SkyboxRenderer {
    pub(crate) fn new(gl: &GL, skybox: Skybox) -> Result<Self, JsValue> {
        let vert_shader = compile_shader(gl, GL::VERTEX_SHADER, SKYBOX_VERTEX_SHADER)
            .map_err(|err| JsValue::from_str(&err))?;
        let frag_shader = compile_shader(gl, GL::FRAGMENT_SHADER, SKYBOX_FRAGMENT_SHADER)
            .map_err(|err| JsValue::from_str(&err))?;
        let program = link_program(gl, &vert_shader, &frag_shader)
            .map_err(|err| JsValue::from_str(&err))?;

        gl.use_program(Some(&program));

        let camera_block_index = gl.get_uniform_block_index(&program, "Camera");
        gl.uniform_block_binding(&program, camera_block_index, 0);

        let vao = gl
            .create_vertex_array()
            .ok_or_else(|| JsValue::from_str("failed to create skybox vertex array"))?;
        gl.bind_vertex_array(Some(&vao));

        let vertex_buffer = gl
            .create_buffer()
            .ok_or_else(|| JsValue::from_str("failed to create skybox vertex buffer"))?;
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&vertex_buffer));

        let vertices = primitive::cube();
        let vertex_array = js_sys::Float32Array::from(vertices.as_slice());
        gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &vertex_array, GL::STATIC_DRAW);

        let position = gl.get_attrib_location(&program, "a_position") as u32;
        gl.enable_vertex_attrib_array(position);
        gl.vertex_attrib_pointer_with_i32(position, 3, GL::FLOAT, false, 24, 0);

        let texture = gl
            .create_texture()
            .ok_or_else(|| JsValue::from_str("failed to create skybox cubemap texture"))?;
        gl.bind_texture(GL::TEXTURE_CUBE_MAP, Some(&texture));
        gl.tex_parameteri(GL::TEXTURE_CUBE_MAP, GL::TEXTURE_MIN_FILTER, GL::LINEAR as i32);
        gl.tex_parameteri(GL::TEXTURE_CUBE_MAP, GL::TEXTURE_MAG_FILTER, GL::LINEAR as i32);
        gl.tex_parameteri(
            GL::TEXTURE_CUBE_MAP,
            GL::TEXTURE_WRAP_S,
            GL::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameteri(
            GL::TEXTURE_CUBE_MAP,
            GL::TEXTURE_WRAP_T,
            GL::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameteri(
            GL::TEXTURE_CUBE_MAP,
            GL::TEXTURE_WRAP_R,
            GL::CLAMP_TO_EDGE as i32,
        );

        let face_targets = [
            GL::TEXTURE_CUBE_MAP_POSITIVE_X,
            GL::TEXTURE_CUBE_MAP_NEGATIVE_X,
            GL::TEXTURE_CUBE_MAP_POSITIVE_Y,
            GL::TEXTURE_CUBE_MAP_NEGATIVE_Y,
            GL::TEXTURE_CUBE_MAP_POSITIVE_Z,
            GL::TEXTURE_CUBE_MAP_NEGATIVE_Z,
        ];

        let fallback_colors = [
            [125, 168, 220, 255],
            [98, 144, 203, 255],
            [160, 194, 234, 255],
            [59, 95, 145, 255],
            [122, 161, 206, 255],
            [75, 113, 166, 255],
        ];

        for (target, color) in face_targets.iter().zip(fallback_colors.iter()) {
            upload_rgba_pixel(gl, &texture, *target, *color)?;
        }

        let mut images = Vec::new();
        let mut load_callbacks = Vec::new();
        let loaded_faces = Rc::new(Cell::new(0_u8));

        match skybox.source {
            SkyboxSource::Cubemap { face_urls } => {
                images = Vec::with_capacity(6);
                load_callbacks = Vec::with_capacity(6);

                for (face_url, target) in face_urls.iter().zip(face_targets.iter()) {
                    let image = HtmlImageElement::new()?;
                    image.set_cross_origin(Some("anonymous"));

                    let gl_for_load = gl.clone();
                    let texture_for_load = texture.clone();
                    let loaded_faces_for_load = Rc::clone(&loaded_faces);
                    let target_for_load = *target;
                    let image_for_load = image.clone();

                    let on_load = Closure::wrap(Box::new(move |_event: web_sys::Event| {
                        gl_for_load.bind_texture(GL::TEXTURE_CUBE_MAP, Some(&texture_for_load));
                        let _ = gl_for_load.tex_image_2d_with_u32_and_u32_and_html_image_element(
                            target_for_load,
                            0,
                            GL::RGBA as i32,
                            GL::RGBA,
                            GL::UNSIGNED_BYTE,
                            &image_for_load,
                        );

                        loaded_faces_for_load.set(loaded_faces_for_load.get().saturating_add(1));
                    }) as Box<dyn FnMut(web_sys::Event)>);
                    image.add_event_listener_with_callback("load", on_load.as_ref().unchecked_ref())?;

                    let on_error = Closure::wrap(Box::new(move |_event: web_sys::Event| {
                        web_sys::console::error_1(&JsValue::from_str("failed to load skybox face"));
                    }) as Box<dyn FnMut(web_sys::Event)>);
                    image.add_event_listener_with_callback("error", on_error.as_ref().unchecked_ref())?;

                    image.set_src(face_url.as_str());
                    images.push(image);
                    load_callbacks.push(on_load);
                    load_callbacks.push(on_error);
                }
            }
            SkyboxSource::Hdr { url, face_size } => {
                let gl_for_load = gl.clone();
                let texture_for_load = texture.clone();
                let url_for_load = url.clone();

                spawn_local(async move {
                    if let Err(err) = load_hdr_to_texture(gl_for_load, texture_for_load, url_for_load, face_size).await {
                        web_sys::console::error_1(&err);
                    }
                });
            }
            SkyboxSource::Equirectangular { url, face_size } => {
                let image = HtmlImageElement::new()?;
                image.set_cross_origin(Some("anonymous"));

                let gl_for_load = gl.clone();
                let texture_for_load = texture.clone();
                let loaded_faces_for_load = Rc::clone(&loaded_faces);
                let image_for_load = image.clone();
                let face_size_for_load = face_size;

                let on_load = Closure::wrap(Box::new(move |_event: web_sys::Event| {
                    let _ = convert_hdri_to_cubemap(
                        &gl_for_load,
                        &texture_for_load,
                        &image_for_load,
                        face_size_for_load,
                    );
                    loaded_faces_for_load.set(6);
                }) as Box<dyn FnMut(web_sys::Event)>);
                image.add_event_listener_with_callback("load", on_load.as_ref().unchecked_ref())?;

                let on_error = Closure::wrap(Box::new(move |_event: web_sys::Event| {
                    web_sys::console::error_1(&JsValue::from_str("failed to load equirectangular skybox source"));
                }) as Box<dyn FnMut(web_sys::Event)>);
                image.add_event_listener_with_callback("error", on_error.as_ref().unchecked_ref())?;

                image.set_src(url.as_str());
                images.push(image);
                load_callbacks.push(on_load);
                load_callbacks.push(on_error);
            }
        }

        gl.bind_vertex_array(None);
        gl.use_program(None);

        Ok(Self {
            gl: gl.clone(),
            program,
            vao,
            _vertex_buffer: vertex_buffer,
            texture,
            _images: images,
            _load_callbacks: load_callbacks,
        })
    }

    pub(crate) fn draw(&self) {
        self.gl.use_program(Some(&self.program));
        self.gl.active_texture(GL::TEXTURE0);
        self.gl.bind_texture(GL::TEXTURE_CUBE_MAP, Some(&self.texture));
        self.gl.bind_vertex_array(Some(&self.vao));

        self.gl.depth_mask(false);
        self.gl.depth_func(GL::LEQUAL);
        self.gl.cull_face(GL::FRONT);

        self.gl.draw_arrays(GL::TRIANGLES, 0, 36);

        self.gl.cull_face(GL::BACK);
        self.gl.depth_func(GL::LESS);
        self.gl.depth_mask(true);
    }
}

fn upload_rgba_pixel(
    gl: &GL,
    texture: &WebGlTexture,
    target: u32,
    rgba: [u8; 4],
) -> Result<(), JsValue> {
    gl.bind_texture(GL::TEXTURE_CUBE_MAP, Some(texture));

    gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
        target,
        0,
        GL::RGBA as i32,
        1,
        1,
        0,
        GL::RGBA,
        GL::UNSIGNED_BYTE,
        Some(&rgba),
    )?;

    Ok(())
}

async fn load_hdr_to_texture(
    gl: GL,
    texture: WebGlTexture,
    url: String,
    face_size: u32,
) -> Result<(), JsValue> {
    let hdr_bytes = fetch_bytes(&url).await?;
    let hdr = decode_radiance_hdr(&hdr_bytes)?;

    let face_targets = [
        GL::TEXTURE_CUBE_MAP_POSITIVE_X,
        GL::TEXTURE_CUBE_MAP_NEGATIVE_X,
        GL::TEXTURE_CUBE_MAP_POSITIVE_Y,
        GL::TEXTURE_CUBE_MAP_NEGATIVE_Y,
        GL::TEXTURE_CUBE_MAP_POSITIVE_Z,
        GL::TEXTURE_CUBE_MAP_NEGATIVE_Z,
    ];

    for (index, target) in face_targets.iter().enumerate() {
        let pixels = generate_hdr_cubemap_face_pixels(&hdr, face_size, index as u32);
        gl.bind_texture(GL::TEXTURE_CUBE_MAP, Some(&texture));
        gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            *target,
            0,
            GL::RGBA as i32,
            face_size as i32,
            face_size as i32,
            0,
            GL::RGBA,
            GL::UNSIGNED_BYTE,
            Some(&pixels),
        )?;
    }

    Ok(())
}

async fn fetch_bytes(url: &str) -> Result<Vec<u8>, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("missing window"))?;
    let response = JsFuture::from(window.fetch_with_str(url)).await?;
    let response: Response = response.dyn_into()?;

    if !response.ok() {
        return Err(JsValue::from_str(&format!("failed to fetch HDR image: {url}")));
    }

    let buffer = JsFuture::from(response.array_buffer()?).await?;
    let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
    Ok(bytes)
}

struct HdrImage {
    width: usize,
    height: usize,
    data: Vec<f32>,
}

fn decode_radiance_hdr(bytes: &[u8]) -> Result<HdrImage, JsValue> {
    let (width, height, orientation, mut offset) = parse_hdr_header(bytes)?;
    let mut data = vec![0.0_f32; width * height * 3];

    if width >= 8 && width <= 0x7fff {
        for scan_y in 0..height {
            if offset + 4 > bytes.len() {
                return Err(JsValue::from_str("unexpected end of HDR data"));
            }

            let header_ok = bytes[offset] == 2 && bytes[offset + 1] == 2;
            let scan_width = ((bytes[offset + 2] as usize) << 8) | (bytes[offset + 3] as usize);
            if header_ok && scan_width == width {
                offset += 4;
                let mut scanline = vec![0_u8; width * 4];

                for channel in 0..4 {
                    let mut x = 0usize;
                    while x < width {
                        if offset >= bytes.len() {
                            return Err(JsValue::from_str("unexpected end of HDR scanline"));
                        }

                        let code = bytes[offset];
                        offset += 1;

                        if code > 128 {
                            let count = (code - 128) as usize;
                            if count == 0 || offset >= bytes.len() || x + count > width {
                                return Err(JsValue::from_str("invalid HDR RLE run"));
                            }
                            let value = bytes[offset];
                            offset += 1;
                            for _ in 0..count {
                                scanline[channel * width + x] = value;
                                x += 1;
                            }
                        } else {
                            let count = code as usize;
                            if count == 0 || offset + count > bytes.len() || x + count > width {
                                return Err(JsValue::from_str("invalid HDR literal run"));
                            }
                            for i in 0..count {
                                scanline[channel * width + x + i] = bytes[offset + i];
                            }
                            offset += count;
                            x += count;
                        }
                    }
                }

                for x in 0..width {
                    let rgbe = [
                        scanline[x],
                        scanline[width + x],
                        scanline[width * 2 + x],
                        scanline[width * 3 + x],
                    ];
                    let rgb = rgbe_to_linear(rgbe);
                    let (dst_x, dst_y) = orientation.remap(x, scan_y, width, height);
                    store_hdr_pixel(&mut data, width, dst_x, dst_y, rgb);
                }
            } else {
                return decode_radiance_hdr_flat(bytes, offset, width, height, orientation);
            }
        }
    } else {
        return decode_radiance_hdr_flat(bytes, offset, width, height, orientation);
    }

    Ok(HdrImage { width, height, data })
}

fn decode_radiance_hdr_flat(
    bytes: &[u8],
    mut offset: usize,
    width: usize,
    height: usize,
    orientation: HdrOrientation,
) -> Result<HdrImage, JsValue> {
    let mut data = vec![0.0_f32; width * height * 3];

    for scan_y in 0..height {
        for x in 0..width {
            if offset + 4 > bytes.len() {
                return Err(JsValue::from_str("unexpected end of flat HDR data"));
            }
            let rgbe = [bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]];
            offset += 4;
            let rgb = rgbe_to_linear(rgbe);
            let (dst_x, dst_y) = orientation.remap(x, scan_y, width, height);
            store_hdr_pixel(&mut data, width, dst_x, dst_y, rgb);
        }
    }

    Ok(HdrImage { width, height, data })
}

fn parse_hdr_header(bytes: &[u8]) -> Result<(usize, usize, HdrOrientation, usize), JsValue> {
    let mut offset = 0usize;
    let mut width = None;
    let mut height = None;
    let mut x_positive = false;
    let mut y_positive = false;

    loop {
        let (line, next_offset) = read_hdr_line(bytes, offset)?;
        offset = next_offset;
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        if line.starts_with('#') || line.starts_with("FORMAT=") || line.starts_with("EXPOSURE=") {
            continue;
        }

        if line.contains('X') && line.contains('Y') {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() != 4 {
                return Err(JsValue::from_str("invalid HDR resolution line"));
            }

            for pair in parts.chunks_exact(2) {
                let axis = pair[0];
                let value: usize = pair[1]
                    .parse()
                    .map_err(|_| JsValue::from_str("invalid HDR resolution number"))?;

                match axis.chars().nth(1) {
                    Some('X') => {
                        width = Some(value);
                        x_positive = axis.starts_with('+');
                    }
                    Some('Y') => {
                        height = Some(value);
                        y_positive = axis.starts_with('+');
                    }
                    _ => return Err(JsValue::from_str("invalid HDR axis token")),
                }
            }

            break;
        }
    }

    Ok((
        width.ok_or_else(|| JsValue::from_str("missing HDR width"))?,
        height.ok_or_else(|| JsValue::from_str("missing HDR height"))?,
        HdrOrientation { x_positive, y_positive },
        offset,
    ))
}

fn read_hdr_line(bytes: &[u8], start: usize) -> Result<(&str, usize), JsValue> {
    if start >= bytes.len() {
        return Err(JsValue::from_str("unexpected end of HDR header"));
    }

    let mut end = start;
    while end < bytes.len() && bytes[end] != b'\n' {
        end += 1;
    }

    let line = std::str::from_utf8(&bytes[start..end])
        .map_err(|_| JsValue::from_str("HDR header is not valid UTF-8"))?;
    let next = if end < bytes.len() { end + 1 } else { end };
    Ok((line.trim_end_matches('\r'), next))
}

#[derive(Clone, Copy)]
struct HdrOrientation {
    x_positive: bool,
    y_positive: bool,
}

impl HdrOrientation {
    fn remap(&self, x: usize, y: usize, width: usize, height: usize) -> (usize, usize) {
        let dst_x = if self.x_positive { x } else { width - 1 - x };
        let dst_y = if self.y_positive { height - 1 - y } else { y };
        (dst_x, dst_y)
    }
}

fn rgbe_to_linear(rgbe: [u8; 4]) -> [f32; 3] {
    if rgbe[3] == 0 {
        [0.0, 0.0, 0.0]
    } else {
        let scale = 2.0_f32.powi(rgbe[3] as i32 - (128 + 8));
        [
            rgbe[0] as f32 * scale,
            rgbe[1] as f32 * scale,
            rgbe[2] as f32 * scale,
        ]
    }
}

fn store_hdr_pixel(data: &mut [f32], width: usize, x: usize, y: usize, rgb: [f32; 3]) {
    let idx = (y * width + x) * 3;
    data[idx] = rgb[0];
    data[idx + 1] = rgb[1];
    data[idx + 2] = rgb[2];
}

fn generate_hdr_cubemap_face_pixels(hdr: &HdrImage, face_size: u32, face_index: u32) -> Vec<u8> {
    let size = face_size.max(1) as usize;
    let mut pixels = vec![0_u8; size * size * 4];

    for y in 0..size {
        for x in 0..size {
            let u = 2.0 * ((x as f32 + 0.5) / size as f32) - 1.0;
            let v = 2.0 * ((y as f32 + 0.5) / size as f32) - 1.0;

            let dir = cubemap_direction(face_index, u, v);
            let color = sample_hdr_equirectangular(hdr, dir);
            let mapped = tone_map(color);

            let idx = (y * size + x) * 4;
            pixels[idx..idx + 4].copy_from_slice(&mapped);
        }
    }

    pixels
}

fn sample_hdr_equirectangular(hdr: &HdrImage, dir: [f32; 3]) -> [f32; 3] {
    let length = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt().max(f32::EPSILON);
    let x = dir[0] / length;
    let y = dir[1] / length;
    let z = dir[2] / length;

    let mut u = 0.5 + z.atan2(x) / (2.0 * PI);
    if u < 0.0 {
        u += 1.0;
    }
    if u > 1.0 {
        u -= 1.0;
    }

    let v = 0.5 - y.clamp(-1.0, 1.0).asin() / PI;

    let px = u * (hdr.width as f32 - 1.0);
    let py = v * (hdr.height as f32 - 1.0);

    bilinear_sample_hdr(hdr, px, py)
}

fn bilinear_sample_hdr(hdr: &HdrImage, x: f32, y: f32) -> [f32; 3] {
    let x0 = x.floor().clamp(0.0, (hdr.width.saturating_sub(1)) as f32) as usize;
    let y0 = y.floor().clamp(0.0, (hdr.height.saturating_sub(1)) as f32) as usize;
    let x1 = (x0 + 1).min(hdr.width.saturating_sub(1));
    let y1 = (y0 + 1).min(hdr.height.saturating_sub(1));

    let tx = x - x0 as f32;
    let ty = y - y0 as f32;

    let c00 = fetch_hdr_rgb(hdr, x0, y0);
    let c10 = fetch_hdr_rgb(hdr, x1, y0);
    let c01 = fetch_hdr_rgb(hdr, x0, y1);
    let c11 = fetch_hdr_rgb(hdr, x1, y1);

    [
        lerp(lerp(c00[0], c10[0], tx), lerp(c01[0], c11[0], tx), ty),
        lerp(lerp(c00[1], c10[1], tx), lerp(c01[1], c11[1], tx), ty),
        lerp(lerp(c00[2], c10[2], tx), lerp(c01[2], c11[2], tx), ty),
    ]
}

fn fetch_hdr_rgb(hdr: &HdrImage, x: usize, y: usize) -> [f32; 3] {
    let idx = (y * hdr.width + x) * 3;
    [hdr.data[idx], hdr.data[idx + 1], hdr.data[idx + 2]]
}

fn tone_map(color: [f32; 3]) -> [u8; 4] {
    let exposure = 1.0;
    let gamma = 1.0 / 2.2;

    let map = |c: f32| -> u8 {
        let mapped = 1.0 - (-c * exposure).exp();
        (mapped.max(0.0).powf(gamma) * 255.0).round().clamp(0.0, 255.0) as u8
    };

    [map(color[0]), map(color[1]), map(color[2]), 255]
}

fn convert_hdri_to_cubemap(
    gl: &GL,
    texture: &WebGlTexture,
    image: &HtmlImageElement,
    face_size: u32,
) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("missing window"))?;
    let document = window.document().ok_or_else(|| JsValue::from_str("missing document"))?;

    let canvas = document
        .create_element("canvas")?
        .dyn_into::<HtmlCanvasElement>()?;
    canvas.set_width(image.width());
    canvas.set_height(image.height());

    let context = canvas
        .get_context("2d")?
        .ok_or_else(|| JsValue::from_str("failed to get 2d canvas context"))?
        .dyn_into::<CanvasRenderingContext2d>()?;
    context.draw_image_with_html_image_element(image, 0.0, 0.0)?;

    let image_data = context.get_image_data(0.0, 0.0, canvas.width() as f64, canvas.height() as f64)?;
    let data = image_data.data().0.to_vec();
    let width = canvas.width() as usize;
    let height = canvas.height() as usize;

    let face_targets = [
        GL::TEXTURE_CUBE_MAP_POSITIVE_X,
        GL::TEXTURE_CUBE_MAP_NEGATIVE_X,
        GL::TEXTURE_CUBE_MAP_POSITIVE_Y,
        GL::TEXTURE_CUBE_MAP_NEGATIVE_Y,
        GL::TEXTURE_CUBE_MAP_POSITIVE_Z,
        GL::TEXTURE_CUBE_MAP_NEGATIVE_Z,
    ];

    for (index, target) in face_targets.iter().enumerate() {
        let pixels = generate_cubemap_face_pixels(&data, width, height, face_size, index as u32);
        gl.bind_texture(GL::TEXTURE_CUBE_MAP, Some(texture));
        gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            *target,
            0,
            GL::RGBA as i32,
            face_size as i32,
            face_size as i32,
            0,
            GL::RGBA,
            GL::UNSIGNED_BYTE,
            Some(&pixels),
        )?;
    }

    Ok(())
}

fn generate_cubemap_face_pixels(
    source: &[u8],
    width: usize,
    height: usize,
    face_size: u32,
    face_index: u32,
) -> Vec<u8> {
    let size = face_size.max(1) as usize;
    let mut pixels = vec![0_u8; size * size * 4];

    for y in 0..size {
        for x in 0..size {
            let u = 2.0 * ((x as f32 + 0.5) / size as f32) - 1.0;
            let v = 2.0 * ((y as f32 + 0.5) / size as f32) - 1.0;

            let dir = cubemap_direction(face_index, u, v);
            let color = sample_equirectangular(source, width, height, dir);

            let idx = (y * size + x) * 4;
            pixels[idx..idx + 4].copy_from_slice(&color);
        }
    }

    pixels
}

fn cubemap_direction(face: u32, u: f32, v: f32) -> [f32; 3] {
    match face {
        0 => [1.0, -v, -u],
        1 => [-1.0, -v, u],
        2 => [u, 1.0, v],
        3 => [u, -1.0, -v],
        4 => [u, -v, 1.0],
        _ => [-u, -v, -1.0],
    }
}

fn sample_equirectangular(source: &[u8], width: usize, height: usize, dir: [f32; 3]) -> [u8; 4] {
    let length = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt().max(f32::EPSILON);
    let x = dir[0] / length;
    let y = dir[1] / length;
    let z = dir[2] / length;

    let mut u = 0.5 + z.atan2(x) / (2.0 * PI);
    if u < 0.0 {
        u += 1.0;
    }
    if u > 1.0 {
        u -= 1.0;
    }

    let v = 0.5 - y.clamp(-1.0, 1.0).asin() / PI;

    let px = u * (width as f32 - 1.0);
    let py = v * (height as f32 - 1.0);

    bilinear_sample_rgba(source, width, height, px, py)
}

fn bilinear_sample_rgba(source: &[u8], width: usize, height: usize, x: f32, y: f32) -> [u8; 4] {
    let x0 = x.floor().clamp(0.0, (width.saturating_sub(1)) as f32) as usize;
    let y0 = y.floor().clamp(0.0, (height.saturating_sub(1)) as f32) as usize;
    let x1 = (x0 + 1).min(width.saturating_sub(1));
    let y1 = (y0 + 1).min(height.saturating_sub(1));

    let tx = x - x0 as f32;
    let ty = y - y0 as f32;

    let c00 = fetch_rgba(source, width, x0, y0);
    let c10 = fetch_rgba(source, width, x1, y0);
    let c01 = fetch_rgba(source, width, x0, y1);
    let c11 = fetch_rgba(source, width, x1, y1);

    let mut out = [0_u8; 4];
    for channel in 0..4 {
        let a = lerp(c00[channel] as f32, c10[channel] as f32, tx);
        let b = lerp(c01[channel] as f32, c11[channel] as f32, tx);
        out[channel] = lerp(a, b, ty).round().clamp(0.0, 255.0) as u8;
    }

    out
}

fn fetch_rgba(source: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
    let idx = (y * width + x) * 4;
    [source[idx], source[idx + 1], source[idx + 2], source[idx + 3]]
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub(crate) fn build_skybox_renderer(gl: &GL, skybox: Skybox) -> Result<SkyboxRenderer, JsValue> {
    SkyboxRenderer::new(gl, skybox)
}

impl Default for Skybox {
    fn default() -> Self {
        Self::cubemap_from_urls([
            "/skybox/px.jpg",
            "/skybox/nx.jpg",
            "/skybox/py.jpg",
            "/skybox/ny.jpg",
            "/skybox/pz.jpg",
            "/skybox/nz.jpg",
        ])
    }
}


