use std::{fmt::Display, path::PathBuf};

use byte_unit::Byte;
use llrt_core::vm::Vm;
use rquickjs::{
    CatchResultExt, CaughtError, Ctx, Exception, Function, Module, Object, Promise, Value,
    prelude::Func,
};

use crate::{
    config::VmOptions,
    error::{Error, Result},
    formats,
    provider::{Flavor, Source, load_flavor},
};

/// Runs a script flavor in a fresh VM and converts its result to a plain value.
pub async fn evaluate(
    config_dir: PathBuf,
    flavor: Flavor,
    options: VmOptions,
) -> Result<formats::Value> {
    // The first installed handler wins, so this replaces Vm's CLI default
    // (which exits the process) with the service's logging policy.
    llrt_core::libs::context::set_spawn_error_handler(|_, error| {
        tracing::error!(%error, "LLRT background task failed");
    });
    let defaults = llrt_core::vm::VmOptions::default();
    let vm = Vm::from_options(llrt_core::vm::VmOptions {
        max_stack_size: options
            .max_stack_size
            .map_or(defaults.max_stack_size, to_usize),
        ..defaults
    })
    .await
    .map_err(|e| Error::ScriptSource(e.into()))?;
    // Set after VM creation so LLRT's own setup is not cut short; the limit
    // still counts that baseline. Runaway scripts fail with "out of memory".
    vm.runtime
        .set_memory_limit(to_usize(options.memory_limit))
        .await;
    // Use the context directly: Vm::run* reports errors by exiting the process.
    vm.ctx
        .async_with(async |ctx| {
            let value = call_entry(&ctx, config_dir, flavor)
                .await
                .catch(&ctx)
                .map_err(script_error)?;
            rquickjs_serde::from_value(value).map_err(|e| Error::ScriptSource(e.into()))
        })
        .await
}

async fn call_entry<'js>(
    ctx: &Ctx<'js>,
    config_dir: PathBuf,
    flavor: Flavor,
) -> rquickjs::Result<Value<'js>> {
    let (module, ready) = Module::declare(
        ctx.clone(),
        "__provider__.mjs",
        include_str!("runtime/bootstrap.mjs"),
    )?
    .eval()?;
    ready.into_future::<()>().await?;
    let entry: Function = module.get("default")?;
    let load = Func::from(move |ctx: Ctx<'js>, name: String| {
        let flavor = load_flavor(&config_dir, &name).map_err(|error| throw(&ctx, error))?;
        flavor_to_js(&ctx, flavor)
    });
    let result: Promise = entry.call((load, flavor_to_js(ctx, flavor)?))?;
    result.into_future().await
}

fn flavor_to_js<'js>(ctx: &Ctx<'js>, flavor: Flavor) -> rquickjs::Result<Object<'js>> {
    let object = Object::new(ctx.clone())?;
    object.set("name", flavor.name)?;
    match flavor.source {
        Source::Script(path) => object.set("module", path.to_string_lossy().as_ref())?,
        Source::Data(value) => object.set(
            "value",
            rquickjs_serde::to_value(ctx.clone(), value).map_err(|error| throw(ctx, error))?,
        )?,
    }
    Ok(object)
}

fn throw(ctx: &Ctx<'_>, error: impl Display) -> rquickjs::Error {
    Exception::throw_message(ctx, &error.to_string())
}

fn script_error(error: CaughtError<'_>) -> Error {
    match error {
        // QuickJS throws `null` when it cannot even allocate the out-of-memory error.
        CaughtError::Value(value) if value.is_null() => {
            Error::Script("Script threw null (likely out of memory)".into())
        }
        error => Error::Script(error.to_string()),
    }
}

fn to_usize(size: Byte) -> usize {
    usize::try_from(size.as_u64()).unwrap_or(usize::MAX)
}
