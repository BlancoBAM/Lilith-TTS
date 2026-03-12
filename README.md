# Lilith-TTS
A standalone, high-performance Text-to-Speech synthesis engine for Lilith Linux, originally part of the Lilim system.

## Features
- **NeuTTS Nano Integration**: Fast and crisp offline voice synthesis using GGUF models.
- **Contextual Memory**: Integrates natively with [Cortex-Mem](https://github.com/sopaco/cortex-mem) to dynamically apply your phonetic rules and pronunciation preferences from your persistent user profile.

## Installation
```bash
cargo build --release
```

## Credits
This project utilizes the incredible [Cortex-Mem framework](https://github.com/sopaco/cortex-mem) for robust, vector-based context and phonetic memory resolution.
