# percept — Implementation Plan

## Overview

`percept` is a Rust CLI that annotates screenshots using OmniParser V2 models (Microsoft's screen parsing tool) and provides computer interaction commands using block IDs instead of pixel coordinates. Built for AI agents that struggle with precise coordinate targeting.

**All inference runs locally in Rust via ONNX Runtime. No Python. No cloud APIs.**

---

## Architecture

```
┌───────────────────────────────────────────────────────────┐
│                    percept (single Rust binary)            │
│                                                           │
│  ┌──────────┐  ┌──────────┐  ┌─────────────────────────┐ │
│  │ Commands │  │  State   │  │    Platform Layer       │ │
│  │ annotate │  │ (blocks  │  │  screenshot capture     │ │
│  │ click    │  │  store)  │  │  mouse/keyboard input   │ │
│  │ type     │  │          │  │  scrolling              │ │
│  │ scroll   │  │          │  │                         │ │
│  │ screenshot│ │          │  │                         │ │
│  └────┬─────┘  └────┬─────┘  └─────────────────────────┘ │
│       │              │                                     │
│  ┌────▼──────────────▼──────────────────────────────────┐ │
│  │           Inference Engine (ort — ONNX Runtime)       │ │
│  │                                                       │ │
│  │  ┌─────────────┐  ┌──────────────┐  ┌─────────────┐ │ │
│  │  │ YOLO v8     │  │ PaddleOCR    │  │ Florence-2  │ │ │
│  │  │ (detection) │  │ (text det +  │  │ (captioning)│ │ │
│  │  │ icon_detect │  │  recognition)│  │  optional   │ │ │
│  │  │  .onnx      │  │  .onnx       │  │  .onnx      │ │ │
│  │  └─────────────┘  └──────────────┘  └─────────────┘ │ │
│  │                                                       │ │
│  │  ┌──────────────────────────────────────────────────┐ │ │
│  │  │ Processing Pipeline (pure Rust)                  │ │ │
│  │  │  image preprocessing · NMS · IOU overlap removal │ │ │
│  │  │  OCR box merging · annotation rendering          │ │ │
│  │  └──────────────────────────────────────────────────┘ │ │
│  └───────────────────────────────────────────────────────┘ │
└───────────────────────────────────────────────────────────┘
         Models: ~/.percept/models/*.onnx
```

---

## Why Pure Rust (No Python Sidecar)

| Concern | Python sidecar | Pure Rust + ONNX |
|---------|---------------|-----------------|
| Install | `cargo install` + Python 3.12 + pip + venv + torch + CUDA toolkit | `cargo install` + download ONNX models |
| Runtime deps | Python interpreter, 4GB+ pip packages | ONNX Runtime (~50MB shared lib, bundled by `ort` crate) |
| Startup latency | 5-10s (Python + model load) | ~1s (ONNX session load, cached after first run) |
| GPU support | PyTorch CUDA | ONNX Runtime CUDA/TensorRT/CoreML/DirectML |
| Distribution | Complex (Rust binary + Python env + model weights) | Single binary + model files |
| CPU fallback | Slow (PyTorch CPU) | Fast (ONNX Runtime optimized CPU kernels) |

---

## Models

All models stored as ONNX files in `~/.percept/models/`. Downloaded during `percept setup`.

| Model | Source | Size | Purpose |
|-------|--------|------|---------|
| `icon_detect.onnx` | OmniParser YOLOv8 (fine-tuned) | ~25MB | Detect interactive UI elements |
| `text_det.onnx` | PaddleOCR DBNet | ~5MB | Detect text regions |
| `text_rec.onnx` | PaddleOCR SVTR/CRNN | ~12MB | Recognize text in detected regions |
| `rec_dictionary.txt` | PaddleOCR | ~200KB | Character dictionary for text recognition |
| `florence2_encoder.onnx` | OmniParser Florence-2 (fine-tuned) | ~200MB | Encode icon crops (optional) |
| `florence2_decoder.onnx` | OmniParser Florence-2 (fine-tuned) | ~200MB | Generate icon captions (optional) |
| `tokenizer.json` | Florence-2 | ~2MB | Tokenizer for caption decoding (optional) |

**Core models** (YOLO + OCR): ~42MB — always required.
**Caption models** (Florence-2): ~402MB — optional, enabled with `--captions` flag.

### Model Conversion

OmniParser ships PyTorch/safetensors weights. One-time conversion to ONNX:

```bash
# We provide pre-converted ONNX models hosted on GitHub Releases / HuggingFace
percept setup                    # downloads pre-converted ONNX models (~42MB)
percept setup --with-captions    # also downloads Florence-2 ONNX models (~444MB)
```

For users who want to convert their own fine-tuned models, a standalone conversion script is provided at `scripts/convert_models.py` (requires Python + torch + ultralytics + optimum, but this is NOT a runtime dependency).

---

## Module Structure

```
src/
  main.rs                  — entry point, CLI definition (clap)
  commands/
    mod.rs
    annotate.rs            — annotate command: run pipeline, save state, output blocks
    click.rs               — click command: look up block, execute click
    type_text.rs           — type command: optional click + type text
    scroll.rs              — scroll command: optional block target + scroll
    screenshot.rs          — screenshot command: capture screen
    setup.rs               — setup command: download ONNX models
  inference/
    mod.rs                 — InferenceEngine: orchestrates the full pipeline
    yolo.rs                — YOLOv8 detection: preprocess, run, NMS postprocess
    ocr.rs                 — PaddleOCR: text detection + recognition
    florence2.rs           — Florence-2 captioning (optional)
    nms.rs                 — non-maximum suppression + IOU utilities
    preprocessing.rs       — shared image resize, normalize, pad operations
  platform/
    mod.rs                 — platform detection + dispatch
    linux.rs               — xdotool/xdg interactions
    macos.rs               — osascript/cliclick interactions
  state.rs                 — block state management (~/.percept/state.json)
  types.rs                 — Block, BoundingBox, AnnotationResult, etc.
scripts/
  convert_models.py        — one-time ONNX conversion (not a runtime dep)
```

---

## Implementation Steps

### Phase 1: Project Scaffolding

1. **Initialize Rust project**
   - `Cargo.toml` with all dependencies (see Dependencies section)
   - Module stubs for all files listed above
   - Basic `main.rs` with clap CLI skeleton

2. **Define core types** (`types.rs`)
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct BoundingBox {
       pub x1: f64, pub y1: f64,  // top-left (normalized 0.0-1.0)
       pub x2: f64, pub y2: f64,  // bottom-right (normalized 0.0-1.0)
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct Block {
       pub id: u32,
       pub bbox: BoundingBox,
       pub label: String,           // OCR text or Florence-2 caption
       pub interactable: bool,      // detected by YOLO = interactable
   }

   pub struct AnnotationResult {
       pub blocks: Vec<Block>,
       pub annotated_image_path: PathBuf,
   }
   ```

3. **Define CLI with clap** (`main.rs`)
   - All commands from README
   - Global flags: `--captions` (enable Florence-2), `--box-threshold <f32>`, `--iou-threshold <f32>`

### Phase 2: Inference Engine — YOLO Detection

4. **ONNX session management** (`inference/mod.rs`)
   - `InferenceEngine` struct that holds loaded ONNX sessions
   - Lazy initialization: models loaded on first `annotate` call
   - Auto-detect CUDA availability, fall back to CPU
   ```rust
   pub struct InferenceEngine {
       yolo_session: Session,
       ocr_det_session: Session,
       ocr_rec_session: Session,
       florence2: Option<Florence2Session>,  // None if --captions not set
   }
   ```

5. **YOLO detection** (`inference/yolo.rs`)
   - **Preprocess**: resize image to 640x640 (letterbox padding), normalize to [0,1], CHW layout, batch dim -> `Array4<f32>` shape `[1, 3, 640, 640]`
   - **Run**: `yolo_session.run(inputs)` -> output tensor shape `[1, 5, 8400]` (x, y, w, h, confidence per anchor)
   - **Postprocess**: confidence threshold filter -> convert xywh to xyxy -> non-maximum suppression -> scale boxes back to original image coordinates -> normalize to [0,1] ratios

6. **NMS utilities** (`inference/nms.rs`)
   - `iou(a: &BoundingBox, b: &BoundingBox) -> f64`
   - `nms(boxes: &mut Vec<Detection>, iou_threshold: f64)`
   - `merge_overlapping(yolo_boxes: &[Detection], ocr_boxes: &[OcrResult], iou_threshold: f64) -> Vec<MergedElement>`

### Phase 3: Inference Engine — OCR

7. **PaddleOCR text detection** (`inference/ocr.rs`)
   - **Preprocess**: resize maintaining aspect ratio (max side 960), normalize with ImageNet mean/std, CHW float32
   - **Run**: `ocr_det_session.run(inputs)` -> probability map
   - **Postprocess** (DB postprocessing): threshold probability map -> binary map -> find contours -> minimum bounding rectangles -> filter by min area -> output text region boxes

8. **PaddleOCR text recognition** (`inference/ocr.rs`)
   - For each detected text region:
     - Crop + perspective-correct the text region from original image
     - Resize to fixed height (48px), variable width, normalize
     - **Run**: `ocr_rec_session.run(inputs)` -> character probability matrix
     - **Postprocess**: CTC greedy decode using character dictionary -> output text string + confidence
   - Batch multiple crops for efficiency

### Phase 4: Pipeline Integration + Annotation

9. **Full pipeline** (`inference/mod.rs` — `InferenceEngine::parse()`)
   ```
   Input image
       |-->  YOLO detection  -> interactive element boxes
       +--> OCR detection   -> text region boxes
                +--> OCR recognition -> text strings
       |
   Merge: IOU-based overlap removal
     - YOLO box overlapping OCR box -> merge (keep YOLO box, attach OCR text, mark interactable)
     - YOLO box without OCR overlap -> "icon" (optionally caption with Florence-2)
     - OCR box without YOLO overlap -> text-only element (mark non-interactable)
       |
   Optional: Florence-2 captioning for unlabeled icons
       |
   Sort by position (top-to-bottom, left-to-right)
   Assign sequential IDs (1, 2, 3, ...)
       |
   Output: Vec<Block>
   ```

10. **Annotation rendering** (`commands/annotate.rs`)
    - Use `image` + `imageproc` crates to draw on the screenshot:
      - Colored bounding box per block
      - Numeric ID label (white text on colored background)
    - Save annotated image to temp path
    - Print block list to stdout

11. **Annotate command** (`commands/annotate.rs`)
    - Read screenshot from `--screenshot <path>`
    - Run `InferenceEngine::parse()`
    - Render annotated image
    - Save block state to `~/.percept/state.json`
    - Output block list + annotated image path

### Phase 5: Screenshot + Interaction Commands

12. **Screenshot command** (`commands/screenshot.rs`, `platform/`)
    - Linux: `scrot` or `grim` (Wayland) via subprocess
    - macOS: `screencapture` via subprocess
    - Save to `--output <path>` or temp file

13. **Click command** (`commands/click.rs`)
    - Load state from `~/.percept/state.json`
    - Look up block by ID -> compute center pixel coordinates
    - Apply optional `--offset <x>,<y>`
    - Execute click via `xdotool` (Linux) or `osascript` (macOS)

14. **Type command** (`commands/type_text.rs`)
    - If `--block <id>`: click block first
    - Type text via `xdotool type` (Linux) or `osascript` (macOS)

15. **Scroll command** (`commands/scroll.rs`)
    - If `--block <id>`: move mouse to block center
    - Execute scroll in `--direction` with `--amount`
    - Via `xdotool` (Linux) or `osascript` (macOS)

### Phase 6: Setup + Polish

16. **Setup command** (`commands/setup.rs`)
    - Download pre-converted ONNX models from GitHub Releases
    - Verify checksums
    - `--with-captions` flag to also download Florence-2 models
    - Report GPU availability (check ONNX Runtime CUDA EP)

17. **Florence-2 captioning** (`inference/florence2.rs`) — optional, lower priority
    - Encoder: crop icon region -> resize to 768x768 -> normalize -> run encoder -> image features
    - Decoder: autoregressive generation loop using `tokenizers` crate for tokenization
    - Only runs for YOLO-detected elements that have no OCR text
    - Activated by `--captions` flag

18. **Config & error handling**
    - Config file at `~/.percept/config.toml` for default thresholds
    - Clear error if models not found ("Run `percept setup` first")
    - Helpful messages for missing platform tools (xdotool, scrot, etc.)

19. **Testing**
    - Unit tests: types, NMS/IOU, state management, CLI parsing
    - Integration tests: pipeline with small test images + expected bounding boxes
    - Mock ONNX sessions for unit testing inference code

---

## Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Language | Rust | Fast CLI, single binary, specified in README |
| ML runtime | `ort` (ONNX Runtime) | Mature, GPU support (CUDA/TensorRT/CoreML), fast CPU fallback |
| Python dependency | **None at runtime** | Single binary + ONNX model files. No Python/pip/venv needed |
| Model format | ONNX | Universal, optimized inference, portable across platforms |
| OCR engine | PaddleOCR ONNX models | Same quality as OmniParser's OCR, runs via same `ort` runtime |
| Icon captioning | Florence-2, optional | ~400MB extra models; most use cases work fine with YOLO+OCR alone |
| State storage | JSON file | Simple, human-readable, no database needed |
| Platform interaction | Subprocess (xdotool, etc.) | Reliable, well-tested tools |

## Dependencies

### Rust (Cargo.toml)

| Crate | Purpose |
|-------|---------|
| `clap` (derive) | CLI argument parsing |
| `ort` | ONNX Runtime bindings — all ML inference |
| `ndarray` | Tensor manipulation for pre/post processing |
| `image` | Image loading, format conversion |
| `imageproc` | Drawing bounding boxes and labels on images |
| `ab_glyph` | Font rendering for annotation labels |
| `serde` + `serde_json` | Serialization (state, config) |
| `tokio` | Async runtime (for setup downloads) |
| `base64` | Image encoding |
| `tempfile` | Temp file management for annotated images |
| `dirs` | Platform config/data directories |
| `toml` | Config file parsing |
| `reqwest` | HTTP client (for model download in `percept setup`) |
| `indicatif` | Progress bars for model download |
| `sha2` | Checksum verification for downloaded models |
| `tokenizers` | Florence-2 tokenization (optional, for captioning) |
| `anyhow` | Error handling |

### System dependencies

- **Linux**: `xdotool` (interaction), `scrot` or `grim` (screenshots)
- **macOS**: None (uses built-in `screencapture` and `osascript`)
- **GPU** (optional): CUDA toolkit for GPU acceleration; CPU works out of the box

---

## Inference Details

### YOLO Pre/Post Processing

**Input**: Image (any size) -> resize to 640x640 with letterbox padding -> normalize [0,1] -> CHW -> `[1,3,640,640]` f32

**Output**: `[1, 5, 8400]` -> transpose to `[8400, 5]` -> each row is `[cx, cy, w, h, confidence]`

**Postprocess**:
1. Filter by confidence > `box_threshold` (default 0.05)
2. Convert center-format to corner-format: `x1=cx-w/2, y1=cy-h/2, x2=cx+w/2, y2=cy+h/2`
3. Scale coordinates back from 640x640 to original image size (accounting for letterbox padding)
4. Apply NMS with `iou_threshold` (default 0.7)
5. Normalize to [0,1] ratios (divide by image width/height)

### PaddleOCR Pre/Post Processing

**Text Detection (DBNet)**:
- Input: resize to max_side=960 maintaining aspect ratio, normalize with `mean=[0.485,0.456,0.406]`, `std=[0.229,0.224,0.225]`, CHW
- Output: probability map same size as input
- Postprocess: threshold (0.3) -> binary bitmap -> find contour polygons -> minimum area rectangles -> filter min_area, expand by ratio 1.5

**Text Recognition (SVTR/CRNN)**:
- Input: crop text region, resize to `3x48x320` (pad/scale width), normalize
- Output: `[1, seq_len, vocab_size]` logits
- Postprocess: argmax per timestep -> CTC decode (collapse repeats, remove blanks) -> map indices to characters via dictionary

### Florence-2 (Optional)

**Encoder**: crop icon -> resize 768x768 -> normalize -> encoder ONNX -> feature tensor
**Decoder**: `<CAPTION>` prompt token -> autoregressive loop (feed previous token + encoder features -> next token logits -> sample) -> decode tokens to text string via `tokenizers`

---

## Execution Order

```
Phase 1 (scaffolding)  ->  Phase 2 (YOLO)  ->  Phase 3 (OCR)
                                                     |
Phase 6 (polish)  <-  Phase 5 (interactions)  <-  Phase 4 (pipeline + annotate)
```

Phase 2 (YOLO) and Phase 3 (OCR) can be developed in parallel.
Phase 4 merges them into the full pipeline.
Phase 5 depends on Phase 4 for state.
Phase 6 includes Florence-2 captioning as an optional enhancement + setup command.
