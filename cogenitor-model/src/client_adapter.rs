//!
//!
//!
//!
//!

trait InnerClient {
    type Error: std::error::Error;
    type InvocationResult;
}

trait ExchangeBuilder<C: InnerClient> {
    fn host_port(self, host: &str, port: u16) -> impl InvocationBuilder<C>;
}

trait Invocator<C: InnerClient> {
    fn invoke(self) -> C::InvocationResult;
}
trait InvocationBuilder<C: InnerClient> {
    fn get(self, path: &str) -> Result<impl RequestBuilder<C>, C::Error>;
    fn put(self, path: &str) -> Result<impl RequestBuilder<C>, C::Error>;
}

trait RequestBuilder<C: InnerClient>: CommonParamBuilder<C> {
    fn header(self, name: &str, value: &str) -> Result<Self, C::Error>;
    fn path_and_query(self, path_and_query: &str) -> Result<Self, C::Error>;
    fn body(self, )
}

#[cfg(test)]
pub fn test_invocation<C: InnerClient>(b: impl ExchangeBuilder<C>) -> Result<(), C::Error> {
    b.host_port("disney.com")
        .get("/foo/bar")?
        .header("Accept", "application/json")?
        ;
    Ok(())
}
