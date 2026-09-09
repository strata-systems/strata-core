---
summary: Download an inference model locally.
mcp_description: Use this when the user wants to download or fetch an inference model so it can run locally, offline.
---

Resolves a catalog name or model spec and downloads the model artifact into the local model directory, returning the resolved local path. A model that is already present is not downloaded again: the command returns its path in every build, with or without network access. Honors `STRATA_MODELS_DIR` for the destination and `STRATA_HF_ENDPOINT` and `STRATA_HF_TOKEN` (or `HF_TOKEN`) for gated HuggingFace repositories. The spec is resolved before anything else is checked: a malformed spec returns `inference.invalid_request`, a cloud provider spec returns `inference.unsupported_operation` (there is nothing to download), and a name that is not in the catalog returns `inference.missing_model`. A missing model is downloaded only when the runtime has network access and the build can download; otherwise the command returns `inference.download_disabled`.
