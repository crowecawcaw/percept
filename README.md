# percept

A CLI tool that annotates screenshots using [OmniParser](https://github.com/microsoft/OmniParser) and provides computer interaction commands that reference annotated blocks instead of pixel coordinates. Built for general-purpose agents that struggle with precise coordinate targeting.

## How it works

1. Run `percept screenshot` to capture and annotate the screen with numbered blocks
2. Use block IDs from the annotation to click, scroll, or interact with elements
3. Repeat — take a new screenshot after each action to get updated block IDs

## Commands

```
percept screenshot --output <path>                             # Take a screenshot, annotate with numbered blocks, save to path
percept screenshot --output <path> --scale <factor>             # Take a screenshot scaled by the given factor
percept screenshot --output <path> --no-annotations            # Take a screenshot without annotations
percept click --block <id>                                     # Click the center of an annotated block
percept click --block <id> --offset <x>,<y>                    # Click with pixel offset relative to block center
percept type --text <string>                                   # Type text at the current cursor position
percept type --block <id> --text <string>                      # Click a block then type text
percept scroll --direction <up|down|left|right>                # Scroll the screen in a direction
percept scroll --block <id> --direction <up|down|left|right>   # Scroll within a specific block
percept scroll --block <id> --amount <pixels>                  # Scroll a specific pixel amount within a block
```

## Install

```
cargo install percept
```

## Acknowledgments

percept uses the [OmniParser](https://github.com/microsoft/OmniParser) icon detection model by Microsoft, which is based on [YOLOv8](https://github.com/ultralytics/ultralytics) by Ultralytics. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for full attribution and license details.
