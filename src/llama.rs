use std::{convert::Infallible, path::Path};

const START_PROMPT: &str = "
You run in a loop of Thought, Action, PAUSE, Observation.
At the end of the loop you output an Answer
Use Thought to describe your thoughts about the question you have been asked.
Use Action to run one of the actions available to you - then return PAUSE.
Observation will be the result of running those actions.

Your available actions are:

calculate:
e.g. calculate: 4 * 7 / 3
Runs a calculation and returns the number - uses Python so be sure to use floating point syntax if necessary

wikipedia:
e.g. wikipedia: Django
Returns a summary from searching Wikipedia

Always look things up on Wikipedia if you have the opportunity to do so.

Example session:

Question: What is the capital of France?
Thought: I should look up France on Wikipedia
Action: wikipedia: France
PAUSE

You will be called again with this:

Observation: France is a country. The capital is Paris.

You then output:

Answer: The capital of France is Paris
";

pub struct Model {
    model: llama_rs::Model,
    vocabulary: llama_rs::Vocabulary,

    cached_inference: Vec<u8>,
}
impl Model {
    pub fn new(path: &Path, context_token_length: usize) -> anyhow::Result<Self> {
        let (model, vocabulary) =
            llama_rs::Model::load(path, context_token_length.try_into()?, |p| {
                dbg!(p);
            })?;

        let mut cached_inference = vec![];
        let mut start_session = model.start_session(64);
        start_session
            .feed_prompt::<Infallible>(
                &model,
                &vocabulary,
                &inference_parameters(),
                START_PROMPT,
                |_| Ok(()),
            )
            .map_err(|e| anyhow::Error::msg(e.to_string()))?;
        unsafe { start_session.get_snapshot() }.write(&mut cached_inference)?;

        Ok(Self {
            model,
            vocabulary,
            cached_inference,
        })
    }

    pub fn session<'a>(&'a mut self) -> anyhow::Result<InferenceSession<'a>> {
        let session = self
            .model
            .session_from_snapshot(llama_rs::InferenceSnapshot::read(
                &mut std::io::Cursor::new(self.cached_inference.as_slice()),
            )?)?;

        Ok(InferenceSession {
            model: self,
            session,
        })
    }
}

pub struct InferenceSession<'a> {
    model: &'a Model,
    session: llama_rs::InferenceSession,
}
impl InferenceSession<'_> {
    pub fn infer(&mut self, question: &str) -> anyhow::Result<()> {
        self.session
            .inference_with_prompt::<Infallible>(
                &self.model.model,
                &self.model.vocabulary,
                &inference_parameters(),
                &format!("Question: {question}\n"),
                None,
                &mut rand::thread_rng(),
                |t| {
                    print!("{t}");

                    Ok(())
                },
            )
            .map_err(|e| anyhow::Error::msg(e.to_string()))?;

        Ok(())
    }
}

fn inference_parameters() -> llama_rs::InferenceParameters {
    llama_rs::InferenceParameters {
        n_threads: 4,
        n_batch: 8,
        top_k: 40,
        top_p: 0.95,
        repeat_penalty: 1.3,
        temp: 0.8,
    }
}
