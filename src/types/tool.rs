pub trait Tool {
    type Context;
    fn apply(&self, context: Self::Context) -> String;
}

pub struct NoContext();

// #[async_trait::async_trait]
// pub trait AsyncTool {
//     async fn apply(&self) -> String;
// }
