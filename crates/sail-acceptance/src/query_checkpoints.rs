//! Actual acknowledged-input prefix validation in certified checkpoint order.
//! Sorting visits evidence only; query bytes and publication order are untouched.
use crate::key_prefix::{HashHost, end};
use crate::query_delivery::Delivered;
use crate::query_prefix::FrozenCommand;
#[cfg(sail_verus)]
use vstd::prelude::*;

sail_spec! {
    pub open spec fn checkpoint_rank<Host: HashHost>(input: &Delivered<Host>, commands: Seq<&FrozenCommand<Host>>,
        order: Seq<usize>, rank: int) -> bool {
        if !input.capture_view().is_some() || rank < 0 || rank >= order.len() { false }
        else if order[rank] >= commands.len() { false }
        else if rank > 0 && order[rank-1] >= commands.len() { false }
        else {
            let bytes=input.capture_view().unwrap(); let command=commands[order[rank] as int];
            let previous=if rank==0 { 0nat } else { commands[order[rank-1] as int].endpoint_view() };
            previous <= command.offset_view() < command.endpoint_view() <= bytes.len()
                && command.command_view() == bytes.subrange(command.offset_view() as int,command.endpoint_view() as int)
                && command.key_view() == Host::digest(end(input.initial_material()+bytes.take(command.endpoint_view() as int),command.endpoint_view() as u64))
        }
    }
    pub open spec fn checkpoints_match<Host: HashHost>(input: &Delivered<Host>, commands: Seq<&FrozenCommand<Host>>,
        order: Seq<usize>, inverse: Seq<usize>) -> bool {
        input.capture_view().is_some() && crate::inputs::valid_permutation(order,inverse,commands.len())
            && forall|rank:int| 0 <= rank < order.len() ==> checkpoint_rank(input,commands,order,rank)
    }
}

sail_verified! {
    /// Constructs a nonempty original parser checkpoint and actual scripted
    /// delivery before checking the complete single-command relation.
    #[allow(clippy::byte_char_slices, reason = "Literal framing bytes make the nonempty constructive stock-Verus witness explicit.")]
    pub fn constructed_checkpoint_control<Host>(host: Host) -> (accepted: bool)
        where (Host: HashHost)
        requires (host.bytes().len() == 0)
        ensures (accepted)
    {
        let bytes=[b'(',b'x',b')'];
        let seed=crate::key_prefix::Prefix::new(host,b"z3",b"pinned",b"binary",b"platform",None);
        let mut parser=crate::query_prefix::QueryPrefix::new(seed.duplicate());
        let mut delivery=crate::query_delivery::Delivery::new_captured(
            crate::query_delivery::ScriptPipe::new(false),seed,3);
        parser.append(&bytes);
        sail_proof! {
            crate::query_prefix::flat_list_frame(seq![b'x']);
            assert(bytes@ =~= seq![b'(']+seq![b'x']+seq![b')']);
            assert(crate::query::symbol_first(bytes@[1]));
            assert(crate::query::trivia_after(Seq::<u8>::empty(),false) == Some(false));
        }
        let Some(borrowed)=parser.command(0,&bytes) else { return false; };
        let command=borrowed.freeze();
        if !parser.hash_tail() || !delivery.write(&bytes) { return false; }
        let Some(input)=delivery.seal(parser.silence_key(),true) else { return false; };
        let commands=[&command]; let order=[0usize]; let inverse=[0usize];
        sail_proof! {
            assert(crate::inputs::valid_permutation(order@,inverse@,1));
            assert(input.capture_view() == Some(bytes@));
            assert(checkpoint_rank(&input,commands@,order@,0));
            assert(checkpoints_match(&input,commands@,order@,inverse@));
        }
        validate_checkpoints(&input,&commands,&order,&inverse)
    }
}

sail_verified! {
    /// Validates every original command span and key against one actual owned
    /// acknowledged tape. The hash advances once through the input, not once
    /// from the beginning per command. Equal digests never imply equal bytes.
    pub fn validate_checkpoints<Host>(input: &Delivered<Host>, commands: &[&FrozenCommand<Host>],
        order: &[usize], inverse: &[usize]) -> (accepted: bool)
        where (Host: HashHost)
        requires (input.capture_consistent())
        ensures (accepted == checkpoints_match(input,commands@,order@,inverse@))
    {
        let Some(bytes)=input.captured_bytes() else { return false; };
        if !crate::inputs::permutation(order,inverse,commands.len()) { return false; }
        let mut prefix=input.initial_prefix().duplicate();
        let mut cursor=0usize; let mut rank=0usize;
        sail_while! { (rank < order.len())
            invariant (rank <= order.len(), crate::inputs::valid_permutation(order@,inverse@,commands@.len()),
                input.capture_view() == Some(bytes@), cursor <= bytes.len(),
                prefix.exact(), prefix.length() == cursor,
                prefix.material() == input.initial_material()+bytes@.take(cursor as int),
                rank == 0 ==> cursor == 0,
                rank > 0 ==> cursor == commands@[order@[rank-1] as int].endpoint_view(),
                forall|i:int| 0 <= i < rank ==> checkpoint_rank(input,commands@,order@,i))
            decreases (order.len()-rank)
            {
                sail_proof! { assert(crate::inputs::permutation_at(order@,inverse@,commands@.len(),rank as int)); }
                let command=commands[order[rank]];
                let start=command.start(); let finish=command.end();
                if start < cursor || start >= finish || finish > bytes.len() { return false; }
                if !crate::inputs::byte_eq(command.command(),&bytes[start..finish]) { return false; }
                prefix.update(&bytes[cursor..finish]);
                let actual=prefix.key(); let proposed=command.key();
                if !crate::inputs::byte_eq(&actual,&proposed) { return false; }
                sail_proof! {
                    assert(bytes@.take(cursor as int)+bytes@.subrange(cursor as int,finish as int) =~= bytes@.take(finish as int));
                    assert(actual =~= proposed);
                    assert(checkpoint_rank(input,commands@,order@,rank as int));
                }
                cursor=finish; rank+=1;
            }
        }
        true
    }
}

sail_verified! {
    /// Missing or repeated evidence cannot pass an alleged complete permutation.
    pub fn incomplete_checkpoint_order_refused<Host>(input: &Delivered<Host>, commands: &[&FrozenCommand<Host>],
        order: &[usize], inverse: &[usize]) -> (refused: bool)
        where (Host: HashHost)
        requires (input.capture_consistent(), !crate::inputs::valid_permutation(order@,inverse@,commands@.len()))
        ensures (refused)
    { !validate_checkpoints(input,commands,order,inverse) }
}

sail_verified! {
    /// The complete declared original checkpoint relation cannot always refuse.
    pub fn nonempty_checkpoint_validation_control<Host>(input: &Delivered<Host>, commands: &[&FrozenCommand<Host>],
        order: &[usize], inverse: &[usize]) -> (accepted: bool)
        where (Host: HashHost)
        requires (input.capture_consistent(), commands@.len() > 0, input.capture_view().is_some(),
            input.capture_view().unwrap().len() > 0, checkpoints_match(input,commands@,order@,inverse@))
        ensures (accepted)
    { validate_checkpoints(input,commands,order,inverse) }
}
