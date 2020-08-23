use crate::commands::classified::block::run_block;
use crate::commands::WholeStreamCommand;
use crate::prelude::*;

use derive_new::new;
use serde::{Deserialize, Serialize};

use crate::context::ScopedCommand;
use nu_errors::ShellError;
use nu_protocol::hir::Block;
use nu_protocol::{Signature, SyntaxShape};

// TODO other derives?
#[derive(Debug, Clone)] // , Serialize, Deserialize)]
pub struct AliasBlock {
    pub block: Block,
    pub arg_shapes: Vec<(String, SyntaxShape)>,
    pub cmd_scopes: Vec<(String, Arc<ScopedCommand>)>,
}

#[derive(new, Clone)]
pub struct AliasCommand {
    name: String,
    block: AliasBlock,
}

#[async_trait]
impl WholeStreamCommand for AliasCommand {
    fn name(&self) -> &str {
        &self.name
    }

    fn signature(&self) -> Signature {
        let mut alias = Signature::build(&self.name);

        for (arg, shape) in &self.block.arg_shapes {
            alias = alias.optional(arg, *shape, "");
        }

        alias
    }

    fn usage(&self) -> &str {
        ""
    }

    async fn run(
        &self,
        args: CommandArgs,
        registry: &CommandRegistry,
    ) -> Result<OutputStream, ShellError> {
        let call_info = args.call_info.clone();
        let mut registry = registry.clone();
        for (cmd, scope) in &self.block.cmd_scopes {
            registry.set_scope(cmd, Arc::clone(scope))
        }

        let mut block = self.block.block.clone();
        block.set_redirect(call_info.args.external_redirection);

        let alias_command = self.clone();
        let mut context = Context::from_args(&args, &registry);
        let input = args.input;

        let mut scope = call_info.scope.clone();
        let evaluated = call_info.evaluate(&registry).await?;
        if let Some(positional) = &evaluated.args.positional {
            for (pos, arg) in positional.iter().enumerate() {
                scope.vars.insert(
                    alias_command.block.arg_shapes[pos].0.to_string(),
                    arg.clone(),
                );
            }
        }

        // FIXME: we need to patch up the spans to point at the top-level error
        Ok(run_block(
            &block,
            &mut context,
            input,
            &scope.it,
            &scope.vars,
            &scope.env,
        )
        .await?
        .to_output_stream())
    }
}
