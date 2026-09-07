---
summary: Report what this binary can do before anything is attempted.
mcp_description: Use this when the user asks whether inference will work, which providers or models are available, or why an inference command failed.
---

Reports the inference facts that are knowable up front: whether this build can execute local models and download them, which providers are compiled in, which of those have an API key and where it was found, and how many catalogued models are already on disk.

Released binaries ship the cloud providers and leave local model execution out, so `local_execution` is false in them and every catalogued local model is unavailable until you build with `--features inference-local`. Knowing that from `status` is the point: previously the only way to find out was to run an operation and read the failure.

`key_source` names the environment variable a key was read from. The key itself is never returned.

The model directory is shared by every database on the machine, so a model downloaded once is available to all of them.
