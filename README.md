# agent-desktop

A CLI tool that annotates screenshots using [OmniParser](https://github.com/microsoft/OmniParser) and provides computer interaction commands that reference annotated blocks instead of pixel coordinates. Built for general-purpose agents that struggle with precise coordinate targeting.

## How it works

1. Run `agent-desktop screenshot` to capture and annotate the screen with numbered blocks
2. Use block IDs from the annotation to click, scroll, or interact with elements
3. Repeat — take a new screenshot after each action to get updated block IDs

## Commands

```
agent-desktop screenshot --output <path>                             # Take a screenshot, annotate with numbered blocks, save to path
agent-desktop screenshot --output <path> --scale <factor>             # Take a screenshot scaled by the given factor
agent-desktop screenshot --output <path> --no-annotations            # Take a screenshot without annotations
agent-desktop click --block <id>                                     # Click the center of an annotated block
agent-desktop click --block <id> --offset <x>,<y>                    # Click with pixel offset relative to block center
agent-desktop type --text <string>                                   # Type text at the current cursor position
agent-desktop type --block <id> --text <string>                      # Click a block then type text
agent-desktop scroll --direction <up|down|left|right>                # Scroll the screen in a direction
agent-desktop scroll --block <id> --direction <up|down|left|right>   # Scroll within a specific block
agent-desktop scroll --block <id> --amount <pixels>                  # Scroll a specific pixel amount within a block
```

## Install

```
cargo install agent-desktop
```
