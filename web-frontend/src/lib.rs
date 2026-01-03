use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{console, HtmlCanvasElement, WebGlProgram, WebGlRenderingContext, WebGlShader};
use serde::{Deserialize, Serialize};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
pub struct App {
    canvas: HtmlCanvasElement,
    gl: Option<WebGlRenderingContext>,
    use_webgl: bool,
}

#[wasm_bindgen]
impl App {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas_id: &str) -> Result<App, JsValue> {
        console_error_panic_hook::set_once();

        let window = web_sys::window().ok_or("no window")?;
        let document = window.document().ok_or("no document")?;
        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or("canvas not found")?
            .dyn_into::<HtmlCanvasElement>()?;

        // Try WebGL2 first, then WebGL1, then fallback to canvas 2D
        let gl = canvas
            .get_context_with_context_options("webgl2", &JsValue::NULL)?
            .or_else(|| {
                canvas
                    .get_context("webgl")
                    .ok()
                    .flatten()
                    .and_then(|ctx| ctx.dyn_into::<web_sys::WebGlRenderingContext>().ok())
            })
            .and_then(|ctx| ctx.dyn_into::<WebGlRenderingContext>().ok());

        let use_webgl = gl.is_some();

        if use_webgl {
            console_log!("Using WebGL rendering");
        } else {
            console_log!("WebGL not available, using Canvas 2D fallback");
        }

        Ok(App {
            canvas,
            gl,
            use_webgl,
        })
    }

    #[wasm_bindgen]
    pub fn init(&mut self) -> Result<(), JsValue> {
        if self.use_webgl {
            self.init_webgl()?;
        } else {
            self.init_canvas_2d()?;
        }
        Ok(())
    }

    fn init_webgl(&mut self) -> Result<(), JsValue> {
        let gl = self.gl.as_ref().ok_or("WebGL context not available")?;

        // Set viewport
        let width = self.canvas.width() as u32;
        let height = self.canvas.height() as u32;
        gl.viewport(0, 0, width as i32, height as i32);

        // Clear with a nice color
        gl.clear_color(0.1, 0.1, 0.15, 1.0);
        gl.clear(WebGlRenderingContext::COLOR_BUFFER_BIT);

        console_log!("WebGL initialized successfully");
        Ok(())
    }

    fn init_canvas_2d(&mut self) -> Result<(), JsValue> {
        let ctx = self
            .canvas
            .get_context("2d")?
            .ok_or("2d context not available")?
            .dyn_into::<web_sys::CanvasRenderingContext2d>()?;

        // Fill with background
        ctx.set_fill_style(&JsValue::from_str("#1a1a26"));
        ctx.fill_rect(0.0, 0.0, self.canvas.width() as f64, self.canvas.height() as f64);

        // Draw welcome text
        ctx.set_fill_style(&JsValue::from_str("#ffffff"));
        ctx.set_font("24px sans-serif");
        ctx.set_text_align("center");
        ctx.set_text_baseline("middle");
        let text = "Buddy Schedule";
        let x = self.canvas.width() as f64 / 2.0;
        let y = self.canvas.height() as f64 / 2.0;
        ctx.fill_text(text, x, y)?;

        console_log!("Canvas 2D initialized successfully");
        Ok(())
    }

    #[wasm_bindgen]
    pub fn render(&self) -> Result<(), JsValue> {
        if self.use_webgl {
            self.render_webgl()?;
        } else {
            self.render_canvas_2d()?;
        }
        Ok(())
    }

    fn render_webgl(&self) -> Result<(), JsValue> {
        let gl = self.gl.as_ref().ok_or("WebGL context not available")?;
        gl.clear(WebGlRenderingContext::COLOR_BUFFER_BIT);
        Ok(())
    }

    fn render_canvas_2d(&self) -> Result<(), JsValue> {
        // Already rendered in init, but can add animation here
        Ok(())
    }

    #[wasm_bindgen]
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), JsValue> {
        self.canvas.set_width(width);
        self.canvas.set_height(height);

        if self.use_webgl {
            if let Some(gl) = &self.gl {
                gl.viewport(0, 0, width as i32, height as i32);
            }
        }

        self.render()?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct ApiResponse<T> {
    data: Option<T>,
    error: Option<String>,
}

#[wasm_bindgen]
pub async fn api_call(path: &str, method: &str, body: Option<String>) -> Result<JsValue, JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let mut opts = web_sys::RequestInit::new();
    opts.method(method);
    opts.mode(web_sys::RequestMode::Cors);

    let mut headers = web_sys::Headers::new()?;
    headers.set("Content-Type", "application/json")?;

    // Get token from localStorage
    if let Ok(Some(token)) = window.local_storage() {
        if let Ok(Some(token_val)) = token.get_item("auth_token") {
            if !token_val.is_empty() {
                headers.set("Authorization", &format!("Bearer {}", token_val))?;
            }
        }
    }

    if let Some(body_str) = body {
        opts.body(Some(&JsValue::from_str(&body_str)));
    }

    opts.headers(&headers);

    let url = format!("/api{}", path);
    let request = web_sys::Request::new_with_str_and_init(&url, &opts)?;
    let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
    let resp: web_sys::Response = resp_value.dyn_into()?;

    let json = JsFuture::from(resp.json()?).await?;
    Ok(json)
}

#[wasm_bindgen]
pub fn set_auth_token(token: &str) {
    if let Ok(Some(storage)) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let _ = storage.set_item("auth_token", token);
    }
}

#[wasm_bindgen]
pub fn get_auth_token() -> String {
    web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
        .and_then(|s| s.get_item("auth_token").ok())
        .flatten()
        .unwrap_or_default()
}

#[wasm_bindgen]
pub fn clear_auth_token() {
    if let Ok(Some(storage)) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let _ = storage.remove_item("auth_token");
    }
}
