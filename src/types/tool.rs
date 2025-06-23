use async_trait::async_trait;
pub trait Tool<C> {
    fn apply(&self, context: C) -> String;
}

#[async_trait]
pub trait AsyncTool<C> {
    async fn apply(&self, context: C) -> Result<String, anyhow::Error>;
}

pub struct AsyncToolWrapper<C>(pub Box<dyn Tool<C> + Send + Sync>);

#[async_trait]
impl<C> AsyncTool<C> for AsyncToolWrapper<C>
where
    C: Send + Sync + 'static,
{
    async fn apply(&self, context: C) -> Result<String, anyhow::Error> {
        let c = Tool::apply(&*self.0, context);
        Ok(c)
    }
}

pub struct NoContext();
