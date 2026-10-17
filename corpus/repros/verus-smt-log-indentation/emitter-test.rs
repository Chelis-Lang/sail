#[cfg(test)]
mod research_log_reproduction {
    use super::Emitter;
    use crate::context::SmtSolver;
    use crate::messages::AirMessageInterface;
    use std::sync::Arc;
    use crate::printer::macro_push_node;
    use crate::node;
    use sise::TreeNode as Node;

    #[test]
    fn smt_file_log_changes_literal_pipe_indentation() {
        let emit = |logged| {
            let log = if logged {
                Some(Box::new(std::io::sink()) as Box<dyn std::io::Write>)
            } else {
                None
            };
            let mut emitter = Emitter::new(
                Arc::new(AirMessageInterface {}), true, true, log, SmtSolver::Z3,
            );
            emitter.log_push();
            emitter.log_node(&node!((assert (and true false))));
            emitter.log_pop();
            emitter.take_pipe_data()
        };
        let off = emit(false);
        let on = emit(true);
        println!("log off literal pipe: {:?}", String::from_utf8(off.clone()).unwrap());
        println!("log on literal pipe: {:?}", String::from_utf8(on.clone()).unwrap());
        assert_ne!(off, on);
    }
}

