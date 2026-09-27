//! Module tools (live C5): the [`Tool`] trait a live module implements, the
//! registry that dispatches to registered tools by name before the built-in
//! ones, the per-ask [`Extensions`] type map they read their state from, and
//! [`Agent::with_tools`]. The built-in tools in [`crate::tools`] are not
//! changed: `tools::execute` asks the registry first and `tools::specs`
//! appends the registered specs after its own.
//!
//! An ask carries its registered tools, its extensions and its time bound in
//! an [`AskScope`] held in a task-local for the duration of the ask, so the
//! tools reach them through the [`ToolContext`] they already receive and no
//! field is added to that type (every evaluation branch builds it literally
//! in its tests).

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, OnceLock, RwLock};

use async_trait::async_trait;
use serde_json::{json, Value};
use vi_core::Result;
use vi_providers::ToolSpec;

use crate::agent::{Agent, AskRequest};
use crate::tools::{ToolCall, ToolContext, ToolOutput};
use crate::until::Bound;

/// A tool a module adds to the agent, next to the built-in ones. Registered
/// through [`Agent::with_tools`] for one agent or [`register_global`] for
/// the whole process (which is what the MCP `tools/list` sees).
#[async_trait]
pub trait Tool: Send + Sync {
    /// What the model sees: name, description and JSON Schema parameters.
    fn spec(&self) -> ToolSpec;
    /// Run one call. An `Err` is turned into an error result for the model,
    /// as a built-in tool's error is; it does not end the ask.
    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<ToolOutput>;
}

/// Per-ask state for module tools, keyed by type: a live module puts its
/// stream handle, portal offset or linked indexes here and its tools read
/// them back with [`Extensions::get`].
#[derive(Clone, Default)]
pub struct Extensions {
    map: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl Extensions {
    /// Empty.
    pub fn new() -> Self {
        Self::default()
    }

    /// Store a value, replacing an earlier one of the same type.
    pub fn insert<T: Any + Send + Sync>(&mut self, value: T) -> &mut Self {
        self.map.insert(TypeId::of::<T>(), Arc::new(value));
        self
    }

    /// Builder form of [`Extensions::insert`].
    pub fn with<T: Any + Send + Sync>(mut self, value: T) -> Self {
        self.insert(value);
        self
    }

    /// The value of a type, if one was stored.
    pub fn get<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        self.map
            .get(&TypeId::of::<T>())
            .and_then(|v| v.clone().downcast::<T>().ok())
    }

    /// Whether a value of the type is stored.
    pub fn contains<T: Any + Send + Sync>(&self) -> bool {
        self.map.contains_key(&TypeId::of::<T>())
    }

    /// Number of values stored.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether nothing is stored.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

impl std::fmt::Debug for Extensions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Extensions")
            .field("len", &self.map.len())
            .finish()
    }
}

/// Tools by name. The first registration of a name wins, so a module cannot
/// shadow one of its own tools by accident; a built-in name is never
/// reached, because the registry is tried first.
#[derive(Clone, Default)]
pub struct ToolRegistry {
    tools: Vec<Arc<dyn Tool>>,
}

impl ToolRegistry {
    /// From a list, dropping later duplicates by name.
    pub fn new(tools: impl IntoIterator<Item = Arc<dyn Tool>>) -> Self {
        let mut out = Self::default();
        for t in tools {
            let name = t.spec().name;
            if !out.tools.iter().any(|x| x.spec().name == name) {
                out.tools.push(t);
            }
        }
        out
    }

    /// The tool owning a name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.iter().find(|t| t.spec().name == name).cloned()
    }

    /// Specs in registration order.
    pub fn specs(&self) -> Vec<ToolSpec> {
        self.tools.iter().map(|t| t.spec()).collect()
    }

    /// Names in registration order.
    pub fn names(&self) -> Vec<String> {
        self.tools.iter().map(|t| t.spec().name).collect()
    }

    /// Whether no tool is registered.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("names", &self.names())
            .finish()
    }
}

/// What one ask carries with it: its registered tools, its extensions and
/// the time bound of its reads and citations (`crate::until`).
#[derive(Debug)]
pub struct AskScope {
    /// Tools registered on the agent.
    pub tools: ToolRegistry,
    /// Module state for this ask.
    pub extensions: Arc<Extensions>,
    /// The `until` clamp.
    pub bound: Arc<Bound>,
}

tokio::task_local! {
    static SCOPE: Arc<AskScope>;
}

/// The scope of the ask running on this task, if any. `None` outside an
/// ask (the MCP server calls the tools directly).
pub fn current() -> Option<Arc<AskScope>> {
    SCOPE.try_with(|s| s.clone()).ok()
}

/// Run `fut` as the body of an ask with `scope`: everything it awaits on
/// this task sees the scope through [`current`].
pub async fn scoped<F: Future>(scope: Arc<AskScope>, fut: F) -> F::Output {
    SCOPE.scope(scope, fut).await
}

fn global() -> &'static RwLock<Vec<Arc<dyn Tool>>> {
    static GLOBAL: OnceLock<RwLock<Vec<Arc<dyn Tool>>>> = OnceLock::new();
    GLOBAL.get_or_init(|| RwLock::new(Vec::new()))
}

/// Register a tool for every ask and every MCP listing in this process.
/// A module server does this once at start-up for the tools it serves
/// through the core router; per-agent tools go through
/// [`Agent::with_tools`]. A name already registered is not replaced.
pub fn register_global(tool: Arc<dyn Tool>) {
    let mut g = global().write().unwrap_or_else(|e| e.into_inner());
    let name = tool.spec().name;
    if !g.iter().any(|t| t.spec().name == name) {
        g.push(tool);
    }
}

/// The process-wide tools, in registration order.
pub fn global_tools() -> Vec<Arc<dyn Tool>> {
    global()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .cloned()
        .collect()
}

/// Specs of every registered tool visible here: the current ask's, then the
/// process-wide ones not already named. `tools::specs` appends them after
/// the built-in specs.
pub fn registered_specs() -> Vec<ToolSpec> {
    let mut out: Vec<ToolSpec> = current().map(|s| s.tools.specs()).unwrap_or_default();
    for t in global_tools() {
        let spec = t.spec();
        if !out.iter().any(|s| s.name == spec.name) {
            out.push(spec);
        }
    }
    out
}

/// The registered tool owning a name: the current ask's first, then the
/// process-wide ones.
pub fn registered(name: &str) -> Option<Arc<dyn Tool>> {
    current()
        .and_then(|s| s.tools.get(name))
        .or_else(|| global_tools().into_iter().find(|t| t.spec().name == name))
}

/// Try the registered tools for a call: `Some` when one owns the name, with
/// its output or an error result the model can read; `None` when the call
/// is for a built-in tool.
pub async fn dispatch(ctx: &ToolContext, call: &ToolCall) -> Option<Result<ToolOutput>> {
    let tool = registered(&call.name)?;
    Some(match tool.execute(ctx, call.args.clone()).await {
        Ok(out) => Ok(out),
        Err(e) => Ok(error_output(&e.to_string())),
    })
}

/// The error result shape the built-in tools use.
pub fn error_output(msg: &str) -> ToolOutput {
    ToolOutput {
        content: json!({"error": msg}).to_string(),
        summary: format!("error: {msg}"),
        ..ToolOutput::default()
    }
}

/// What an agent carries for its asks beyond the core fields: the tools of
/// [`Agent::with_tools`] and the extensions of [`Agent::with_extensions`].
#[derive(Default)]
pub struct AgentExt {
    /// Registered tools, in order.
    pub tools: Vec<Arc<dyn Tool>>,
    /// Module state handed to every ask.
    pub extensions: Extensions,
}

impl std::fmt::Debug for AgentExt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentExt")
            .field("tools", &self.tools.len())
            .field("extensions", &self.extensions)
            .finish()
    }
}

impl AgentExt {
    /// The scope of one ask: the agent's tools and extensions plus the
    /// request's bound.
    pub fn scope_for(&self, req: &AskRequest) -> Arc<AskScope> {
        Arc::new(AskScope {
            tools: ToolRegistry::new(self.tools.iter().cloned()),
            extensions: Arc::new(self.extensions.clone()),
            bound: Arc::new(Bound::new(req.until)),
        })
    }
}

impl Agent {
    /// Add module tools to this agent: appended to the built-in specs the
    /// model sees and dispatched by name before the built-in `match`.
    pub fn with_tools(mut self, tools: Vec<Arc<dyn Tool>>) -> Self {
        self.ext.tools.extend(tools);
        self
    }

    /// State for the module tools of every ask this agent runs, read back
    /// with [`ToolContext::extensions`].
    pub fn with_extensions(mut self, extensions: Extensions) -> Self {
        self.ext.extensions = extensions;
        self
    }
}

impl ToolContext {
    /// The extensions of the ask this call belongs to; empty outside an ask.
    pub fn extensions(&self) -> Arc<Extensions> {
        current().map(|s| s.extensions.clone()).unwrap_or_default()
    }

    /// The time bound of the ask this call belongs to, when it runs inside
    /// one.
    pub fn bound(&self) -> Option<Arc<Bound>> {
        current().map(|s| s.bound.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Echo;

    #[async_trait]
    impl Tool for Echo {
        fn spec(&self) -> ToolSpec {
            ToolSpec {
                name: "echo".into(),
                description: "echo".into(),
                parameters: json!({"type": "object"}),
            }
        }

        async fn execute(&self, _ctx: &ToolContext, args: Value) -> Result<ToolOutput> {
            Ok(ToolOutput {
                content: args.to_string(),
                summary: "echoed".into(),
                ..ToolOutput::default()
            })
        }
    }

    #[derive(Debug, PartialEq)]
    struct Head(f64);

    #[test]
    fn extensions_are_a_type_map() {
        let ext = Extensions::new().with(Head(4360.5)).with(7u32);
        assert_eq!(*ext.get::<Head>().unwrap(), Head(4360.5));
        assert_eq!(*ext.get::<u32>().unwrap(), 7);
        assert!(ext.get::<String>().is_none());
        assert!(ext.contains::<Head>() && !ext.contains::<i64>());
        assert_eq!(ext.len(), 2);
        let mut ext = ext;
        ext.insert(Head(1.0));
        assert_eq!(*ext.get::<Head>().unwrap(), Head(1.0));
        assert_eq!(ext.len(), 2);
    }

    #[test]
    fn registry_keeps_the_first_of_a_name() {
        let r = ToolRegistry::new(vec![
            Arc::new(Echo) as Arc<dyn Tool>,
            Arc::new(Echo) as Arc<dyn Tool>,
        ]);
        assert_eq!(r.names(), vec!["echo".to_string()]);
        assert!(r.get("echo").is_some() && r.get("nope").is_none());
        assert_eq!(r.specs().len(), 1);
        assert!(ToolRegistry::default().is_empty());
    }

    #[tokio::test]
    async fn scope_is_visible_inside_and_absent_outside() {
        assert!(current().is_none());
        let scope = Arc::new(AskScope {
            tools: ToolRegistry::new(vec![Arc::new(Echo) as Arc<dyn Tool>]),
            extensions: Arc::new(Extensions::new().with(Head(2.0))),
            bound: Arc::new(Bound::new(Some(60.0))),
        });
        scoped(scope, async {
            let s = current().unwrap();
            assert_eq!(s.tools.names(), vec!["echo".to_string()]);
            assert_eq!(*s.extensions.get::<Head>().unwrap(), Head(2.0));
            assert_eq!(s.bound.until(), Some(60.0));
            assert!(registered_specs().iter().any(|t| t.name == "echo"));
            assert!(registered("echo").is_some());
        })
        .await;
        assert!(current().is_none());
        assert!(registered("echo").is_none() || !global_tools().is_empty());
    }
}
