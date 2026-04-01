# Alloy

This project is a small Rust + WebAssembly WebGL2 demo.

## Build

Generate the browser package with:

```powershell
wasm-pack build --target web
```

## Serve

Run the local Python server from the project root:

```powershell
python serve.py
```

Then open:

```text
http://127.0.0.1:8000/
```

The server sets explicit MIME types for `.js` and `.wasm` files so the browser can load `pkg/alloy.js` without a blocked MIME-type error.

## Camera controls

- `Ctrl + Left Mouse Drag`: orbit
- `Ctrl + Middle Mouse Drag`: pan
- `Ctrl + Right Mouse Drag` or mouse wheel: dolly/zoom
- `F`: toggle freeflight mode
- `W / A / S / D`: move in freeflight
- `Space`: move up in freeflight
- `Shift`: move down in freeflight

Note: `Ctrl + wheel` can still clash with browser zoom behavior on some browsers/platforms, but the canvas listeners prevent the default wheel/context-menu actions where possible.

## Skybox

The renderer now supports a cubemap skybox that is uploaded once and drawn before the instanced batcher.

The expected face order is:

1. `+X`
2. `-X`
3. `+Y`
4. `-Y`
5. `+Z`
6. `-Z`

Example usage:

```rust
use crate::{canvas, Phong, Skybox};

canvas()?
	.skybox(Skybox::hdri_from_url("/skybox/studio.png"))
	.shading(Phong)
	.start()?;
```

This HDRI loader converts a browser-decodable equirectangular image into a cubemap once at load time, then keeps the draw pass unchanged.

If you already have six cubemap faces, `Skybox::cubemap_from_urls([...])` still works too.

