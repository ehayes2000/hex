use async_trait::async_trait;
pub trait Tool {
    type Context;
    fn apply(&self, context: Self::Context) -> String;
}

#[async_trait]
pub trait AsyncTool {
    type Context;
    async fn apply(&self, context: Self::Context) -> Result<String, anyhow::Error>;
}

pub struct AsyncToolWrapper<C> {
    pub tool: Box<dyn Tool<Context = C> + Send + Sync>,
}

#[async_trait]
impl<C> AsyncTool for AsyncToolWrapper<C>
where
    C: Send + Sync,
{
    type Context = C;
    async fn apply(&self, context: Self::Context) -> Result<String, anyhow::Error> {
        Ok(self.tool.apply(context))
    }
}

pub struct NoContext();
