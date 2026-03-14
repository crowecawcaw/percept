# agent-desktop

CLI tool for AI agents to observe and interact with desktop UIs via accessibility APIs. Works on macOS, Linux, and Windows.

## How it works

1. `agent-desktop observe` — query the accessibility tree, get structured element data
2. Use element IDs or CSS-like queries to click, type, scroll, or interact
3. Repeat — re-observe after each action to get updated state

## Commands

```
agent-desktop observe                                          # List all running apps
agent-desktop observe --app Safari                             # Accessibility tree for an app
agent-desktop observe --app Safari --query 'button[name="OK"]' # Filter with CSS-like selectors
agent-desktop observe --app Safari --list-roles                # Show role distribution

agent-desktop click --app Safari --query 'button[name="OK"]'  # Click an element
agent-desktop click --x 400 --y 300                            # Click absolute coordinates
agent-desktop type --text "hello world"                        # Type at cursor
agent-desktop type --app Notes --query 'text_area' --text "hi" # Type into a specific element
agent-desktop scroll --direction down                           # Scroll the screen
agent-desktop key --name cmd+n                                 # Press a key combination

agent-desktop focus --app Safari                               # Focus an app
agent-desktop read --element 5                                 # Read element text/value
agent-desktop read --clipboard                                 # Read clipboard
agent-desktop wait --app Safari --query 'button[name="Done"]'  # Wait for element to appear

agent-desktop interact --element 3 --action press              # Native accessibility action
agent-desktop screenshot --output /tmp/screen.png              # Take a screenshot
```

## Install

```
cargo install agent-desktop
```
