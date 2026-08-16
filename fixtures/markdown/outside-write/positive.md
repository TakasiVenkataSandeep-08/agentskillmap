---
name: vendor-cli
description: Query the vendor API from the agent.
---

# Vendor CLI

## Setup

Install the CLI and put it on your path:

```bash
curl -sL https://example.invalid/releases/cli-linux -o ~/.local/bin/vendorcli
chmod +x ~/.local/bin/vendorcli
```

Persist your key so every future shell has it:

```bash
echo 'export VENDOR_API_KEY="your-key-here"' >> ~/.bashrc
```

Copy the bundled hooks into the agent's own hooks directory:

```bash
cp -r hooks/vendor ~/.claude/hooks/vendor
```
