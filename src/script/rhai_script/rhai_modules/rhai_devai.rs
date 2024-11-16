//! Defines the `devai` module, used in the rhai engine
//!
//! ---
//!
//! ## RHAI documentation
//! The `devai` module exposes functions that modify the default flow of the
//! devai parser and script runner.
//!
//! ### Functions
//! * `devai::skip() -> SkipDict`
//! * `devai::skip(reason: string) -> SkipDict`
//! * `devai::before_all_response(data: {inputs?: [], before_all?: any}) -> BeforeAllResponseDict`
//! * `devai::run(cmd_agent: &str) -> {outputs: null | any[], after_all: null | any}`
//! * `devai::run(cmd_agent: &str, inputs: Vec<Dynamic>) ->  {outputs: null | any[], after_all: null | any}`
//!
//! Note: the SkipDict and BeforeAllResponseDict are not really important, as it is for the internals to treat those return values appropriately.

use crate::agent::find_agent;
use crate::run::{run_command_agent, RuntimeContext};
use crate::run::{PathResolver, RunBaseOptions};
use crate::script::rhai_script::dynamic_helpers::{dynamics_to_values, value_to_dynamic};
use crate::script::{DynamicMap, IntoDynamic};
use crate::Error;
use rhai::plugin::RhaiResult;
use rhai::{Dynamic, FuncRegistration, Module};
use serde_json::json;

pub fn rhai_module(runtime_context: &RuntimeContext) -> Module {
	// Create a module for text functions
	let mut module = Module::new();

	FuncRegistration::new("skip")
		.in_global_namespace()
		.set_into_module(&mut module, skip);

	FuncRegistration::new("skip")
		.in_global_namespace()
		.set_into_module(&mut module, skip_with_reason);

	FuncRegistration::new("before_all_response")
		.in_global_namespace()
		.set_into_module(&mut module, before_all_response);

	let ctx = runtime_context.clone();
	FuncRegistration::new("run")
		.in_global_namespace()
		.set_into_module(&mut module, move |cmd_agent: &str| {
			run_with_inputs(&ctx, cmd_agent, None)
		});

	let ctx = runtime_context.clone();
	FuncRegistration::new("run")
		.in_global_namespace()
		.set_into_module(&mut module, move |cmd_agent: &str, inputs: Vec<Dynamic>| {
			run_with_inputs(&ctx, cmd_agent, Some(inputs))
		});

	module
}

// region:    --- run...

/// ## RHAI Documentation
/// ```rhai
/// run(cmd_agent: &str) -> {outputs: null | any[], after_all: null | any}
/// run(cmd_agent: &str, inputs: Vec<Dynamic>) -> {outputs: null | any[], after_all: null | any}
/// ```
///
/// This function is used to execute a command agent with optional inputs.
/// It returns the result of the command execution.
///
/// for example, in # Data rhai code block:
///
/// ```rhai
/// let result = devai::run("./agent-script/agent-hello.devai", ["one", "two"]);
/// ```
fn run_with_inputs(ctx: &RuntimeContext, cmd_agent: &str, inputs: Option<Vec<Dynamic>>) -> RhaiResult {
	let inputs = inputs.map(dynamics_to_values).transpose()?;
	// TODO: Might want to reuse the current one
	let agent = find_agent(cmd_agent, ctx.dir_context(), PathResolver::DevaiParentDir)?;

	let rt = tokio::runtime::Handle::try_current().map_err(Error::TokioTryCurrent)?;

	// Note: Require to have
	let runtime = ctx.get_runtime()?;
	let res = tokio::task::block_in_place(|| {
		rt.block_on(async { run_command_agent(&runtime, &agent, inputs, &RunBaseOptions::default(), true).await })
	})?;

	let res =
		serde_json::to_value(res).map_err(|err| Error::cc("devai::run, failed to result convert to json", err))?;

	let rhai_res = value_to_dynamic(&res);

	Ok(rhai_res)
}

// endregion: --- run...

// region:    --- before_all_response

/// ## RHAI Documentation
/// ```rhai
/// before_all_response(data: {inputs?: [], before_all?: any}) -> BeforeAllResponseDict
/// ```
///
/// This function is used to prepare a response before all other operations.
/// It takes an object with optional fields:
///
/// for example, this will override the inputs send to this agent
///
/// ```rhai
/// let response = devai::before_all_response(#{
///     inputs: ["input1", "input2"]
/// });
/// ```
///
///
fn before_all_response(data: Dynamic) -> RhaiResult {
	// validate it is a map
	let data = DynamicMap::from_dynamic(data)
		.map_err(|err| crate::Error::cc("devai::before_all_response take a object map only", err))?;

	// Now building the following structure.
	// Note: The data send by the user will be assume to be of right format for now
	//       As the valuation happen later at the DevaiCustom level
	//	"_devai_": {
	//		"kind": "BeforeAllResponse",
	//		"data": {
	//			"inputs": ["A", "B", 123],
	//			"before_all": "Some before all data"
	//		}
	//	}

	// now build the new
	let custom = DynamicMap::default().insert("kind", "BeforeAllResponse").insert("data", data);
	let mut res = rhai::Map::new();
	res.insert("_devai_".into(), custom.into());

	Ok(res.into())
}

// endregion: --- before_all_response

// region:    --- skip

/// ## RHAI Documentation
/// ```rhai
/// skip() -> SkipDict
/// ```
///
/// This is to be used in the `# Data` and  section to return a devai skip action so that the input is not
/// included in the next flow (instruction > AI > data)
///
/// for example, in # Data rhai code block:
///
/// ```rhai
/// if input.name == "mod.rs" {
///   return devai::skip();
/// }
/// ```
fn skip() -> RhaiResult {
	// TODO: need to create the Dynamic directly,
	//       no need to passthrough json -> Dynamic -> json later
	let res = json!({
		"_devai_": {
			"kind": "Skip"
		}
	});
	let res = value_to_dynamic(&res);

	Ok(res)
}

/// ## RHAI Documentation
/// ```rhai
/// skip(reason: string) -> SkipDict
/// ```
///
/// This is to be used in the `# Data` section to return a devai skip action so that the input is not
/// included in the next flow (instruction > AI > data).
///
/// This `skip` function takes a reason so that it get printed.
///
/// for example, in # Data rhai code block:
///
/// ```rhai
/// if input.name == "mod.rs" {
///   return devai::skip("mod.rs does not need to be process by this agent");
/// }
/// ```
fn skip_with_reason(reason: &str) -> RhaiResult {
	let res = json!({
		"_devai_": {
			"kind": "Skip",
			"data": {
				"reason": reason
			}
		}
	});
	let res = value_to_dynamic(&res);

	Ok(res)
}

// endregion: --- skip

// region:    --- Tests

#[cfg(test)]
mod tests {
	type Error = Box<dyn std::error::Error>;
	type Result<T> = core::result::Result<T, Error>; // For tests.

	use crate::_test_support::run_reflective_agent;
	use serde_json::from_value;
	use value_ext::JsonValueExt;

	// Note: multi_thread required, because rhai devai::run is a sync calling a async.
	#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
	async fn test_rhai_devai_run_simple() -> Result<()> {
		let res = run_reflective_agent(
			r#"return devai::run("./agent-script/agent-hello.devai", ["one", "two"])"#,
			None,
		)
		.await;

		// NOTE: apparently when multi thread, need to print error
		let mut res = match res {
			Ok(res) => res,
			Err(err) => {
				panic!("test_rhai_devai_run_simple ERROR: {err}");
			}
		};

		let vals: Vec<String> = from_value(res.x_take("outputs")?)?;

		assert_eq!(
			vals,
			["hello 'one' from agent-hello.devai", "hello 'two' from agent-hello.devai"]
		);
		Ok(())
	}
}

// endregion: --- Tests
