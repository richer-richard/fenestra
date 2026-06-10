# fenestra architecture

Decisions are recorded here as they are made, milestone by milestone.

## Workspace

```
fenestra/        facade: prelude, re-exports, run(), examples/
fenestra-core/   Element IR, Style, Theme and tokens, style resolution,
                 taffy integration, parley integration, vello scene build,
                 hit testing, FrameState, transition engine
fenestra-shell/  winit + wgpu runner (windowed) and the headless renderer
fenestra-kit/    design-system widgets built only on core's public API
```

`fenestra-core` and `fenestra-kit` build and test with no window: tests use an
offscreen wgpu adapter via `fenestra-shell`'s headless renderer (a
dev-dependency only). `fenestra-shell` isolates all OS glue.

## Dependency versions (resolved 2026-06-10, latest stable)

winit 0.30.13, wgpu 29.0.3, vello 0.9.0, kurbo 0.13.1, peniko 0.6.1,
parley 0.10.0, fontique 0.10.0, taffy 0.10.1, color 0.3.3, arboard 3.6.1,
image 0.25.10, insta 1.47.2. The tree is mutually consistent: vello 0.9
requires wgpu ^29.0.3 and peniko ^0.6.1; parley 0.10 requires fontique ^0.10;
peniko 0.6.1 requires color ^0.3.3 and kurbo ^0.13.1.

vello 0.9 is the classic compute-shader renderer (`vello_encoding` +
`vello_shaders` + wgpu), not the newer sparse-strips crates (`vello_cpu`,
`vello_hybrid`).

## M0 decisions

- **Surface presentation.** vello renders with a compute shader and cannot
  bind most surface textures directly. We follow vello's own recommended
  pattern: render into the intermediate `Rgba8Unorm` STORAGE_BINDING texture
  that `vello::util::RenderSurface` maintains, then blit to the surface with
  `wgpu::util::TextureBlitter`. wgpu 29's `get_current_texture()` returns a
  `CurrentSurfaceTexture` enum (no longer `Result`); `Outdated`/`Suboptimal`
  reconfigure and retry, `Occluded`/`Timeout` just request a new redraw.
- **Headless readback.** Offscreen target texture is `Rgba8Unorm` with
  `STORAGE_BINDING | COPY_SRC`, copied to a `MAP_READ` buffer with rows padded
  to 256 bytes, then unpadded into an `image::RgbaImage`. We block with
  `std::sync::mpsc` + `device.poll(PollType::wait_indefinitely())` instead of
  pulling in `futures-intrusive`.
- **Antialiasing.** `AaConfig::Area` everywhere (window and headless), with
  renderers built `AaSupport::area_only()`. Area AA is deterministic and
  software-rasterizer friendly (Mesa lavapipe in CI), and using the same mode
  in both paths keeps on-screen output identical to the golden images.
- **Logical coordinates.** Frames are built in logical pixels into a fragment
  `Scene`, then appended to the root scene under `Affine::scale(scale_factor)`.
  Headless renders at scale 1.0, so logical == physical there.
- **Dev profile.** vello/wgpu are unusably slow unoptimized;
  `[profile.dev.package."*"] opt-level = 2` keeps our crates fast to compile
  while dependencies stay fast to run.
- **Shadow blur mapping.** M0 uses a provisional `std_dev = blur / 2`; M1
  calibrates against a CSS box-shadow reference and locks it with a snapshot.
