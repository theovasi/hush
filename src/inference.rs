use ouroboros::self_referencing;
use std::path::Path;
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

#[self_referencing]
pub struct ModelState {
    whisper_context: WhisperContext,
    #[borrows(whisper_context)]
    pub whisper_state: WhisperState,
}

pub struct WhisperModel {
    state: ModelState,
}

impl WhisperModel {
    pub fn new(model_path: &Path) -> Result<Self, anyhow::Error> {
        let path_str = model_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid model path"))?;

        let context =
            WhisperContext::new_with_params(path_str, WhisperContextParameters::default())?;

        let model_state = ModelStateBuilder {
            whisper_context: context,
            whisper_state_builder: |ctx| {
                ctx.create_state().expect("Failed to create whisper state")
            },
        }
        .build();

        Ok(Self { state: model_state })
    }

    pub fn transcribe(
        &mut self,
        samples: &[f32],
        language: Option<&str>,
    ) -> Result<Vec<String>, anyhow::Error> {
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        if let Some(lang) = language {
            params.set_language(Some(lang));
        } else {
            params.set_language(None);
        }

        self.state.with_whisper_state_mut(|whisper_state| {
            whisper_state.full(params, samples)?;

            let n_segments = whisper_state.full_n_segments();
            let mut segments = Vec::with_capacity(n_segments as usize);

            for i in 0..n_segments {
                if let Some(segment) = whisper_state.get_segment(i) {
                    segments.push(segment.to_str().unwrap_or("").to_string());
                }
            }

            Ok(segments)
        })
    }
}
