//! Thin WebGL2 helpers: compile shaders, link programs, upload f32 buffers.

use wasm_bindgen::JsValue;
use web_sys::{
    WebGl2RenderingContext as GL, WebGlBuffer, WebGlProgram, WebGlShader,
    WebGlVertexArrayObject,
};

pub fn compile_shader(gl: &GL, kind: u32, src: &str) -> Result<WebGlShader, JsValue> {
    let shader = gl
        .create_shader(kind)
        .ok_or_else(|| JsValue::from_str("create_shader returned null"))?;
    gl.shader_source(&shader, src);
    gl.compile_shader(&shader);
    if gl
        .get_shader_parameter(&shader, GL::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        let log = gl
            .get_shader_info_log(&shader)
            .unwrap_or_else(|| "unknown shader error".into());
        Err(JsValue::from_str(&format!("shader compile error: {log}")))
    }
}

pub fn link_program(gl: &GL, vert: &str, frag: &str) -> Result<WebGlProgram, JsValue> {
    let vs = compile_shader(gl, GL::VERTEX_SHADER, vert)?;
    let fs = compile_shader(gl, GL::FRAGMENT_SHADER, frag)?;
    let program = gl
        .create_program()
        .ok_or_else(|| JsValue::from_str("create_program returned null"))?;
    gl.attach_shader(&program, &vs);
    gl.attach_shader(&program, &fs);
    gl.link_program(&program);
    if gl
        .get_program_parameter(&program, GL::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        // Shaders can be detached/deleted once linked.
        gl.detach_shader(&program, &vs);
        gl.detach_shader(&program, &fs);
        gl.delete_shader(Some(&vs));
        gl.delete_shader(Some(&fs));
        Ok(program)
    } else {
        let log = gl
            .get_program_info_log(&program)
            .unwrap_or_else(|| "unknown link error".into());
        Err(JsValue::from_str(&format!("program link error: {log}")))
    }
}

/// Create a STATIC_DRAW array buffer from f32 data.
pub fn make_f32_buffer(gl: &GL, data: &[f32]) -> Result<WebGlBuffer, JsValue> {
    let buf = gl
        .create_buffer()
        .ok_or_else(|| JsValue::from_str("create_buffer returned null"))?;
    gl.bind_buffer(GL::ARRAY_BUFFER, Some(&buf));
    // SAFETY: the Float32Array view borrows wasm linear memory for the duration
    // of the buffer_data call only; no allocation happens in between.
    unsafe {
        let view = js_sys::Float32Array::view(data);
        gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &view, GL::STATIC_DRAW);
    }
    Ok(buf)
}

pub fn make_vao(gl: &GL) -> Result<WebGlVertexArrayObject, JsValue> {
    gl.create_vertex_array()
        .ok_or_else(|| JsValue::from_str("create_vertex_array returned null"))
}
