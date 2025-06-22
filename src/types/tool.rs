pub trait Tool {
    type Context;
    fn apply(&self, context: Self::Context) -> String;
}

#[async_trait::async_trait]
pub trait AsyncTool {
    type Context;
    async fn apply(&self, context: Self::Context) -> ToolCallResponse;
}

pub enum ToolCallResponse {
    OkNone,
    OkContext(String),
    FailNone,
    Fail(String),
}

pub struct NoContext();
