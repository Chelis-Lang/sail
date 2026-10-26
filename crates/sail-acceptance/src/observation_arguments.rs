//! Private operational argv envelope, stripped before every ordinary launcher parse.
//! Both builds compile the same planner, refusal checks and exact restoration loops.
#[cfg(sail_verus)]
use vstd::prelude::*;

sail_spec! {
    pub open spec fn string_bytes(value: &str) -> Seq<u8> { vstd::utf8::encode_utf8(value@) }
    pub open spec fn strings(values: Seq<String>) -> Seq<Seq<char>> { values.map_values(|s: String| s@) }
    pub open spec fn flag_bytes(argument: Seq<u8>, flag: Seq<u8>) -> bool {
        argument.len() >= flag.len() && argument.take(flag.len() as int) == flag
            && (argument.len() == flag.len() || argument[flag.len() as int] == 61u8)
    }
    pub open spec fn private_chars(value: Seq<char>) -> bool {
        flag_bytes(vstd::utf8::encode_utf8(value), vstd::utf8::encode_utf8("--sail-observation-request"@))
            || flag_bytes(vstd::utf8::encode_utf8(value), vstd::utf8::encode_utf8("--sail-observation-dir"@))
            || flag_bytes(vstd::utf8::encode_utf8(value), vstd::utf8::encode_utf8("--sail-observation-envelope-v1"@))
    }
    pub open spec fn private_option(value: &str) -> bool { private_chars(value@) }
    pub open spec fn usable_chars(path: Seq<char>) -> bool {
        let bytes = vstd::utf8::encode_utf8(path);
        bytes.len() > 0 && bytes[0] != 45u8 && forall|i: int| 0 <= i < bytes.len() ==> bytes[i] != 0u8
    }
    pub open spec fn usable_path(path: &str) -> bool { usable_chars(path@) }
    pub open spec fn can_plan(arguments: Seq<String>, request: &str, directory: &str) -> bool {
        arguments.len() <= usize::MAX - 4 && usable_path(request) && usable_path(directory)
            && forall|i: int| 0 <= i < arguments.len() ==> !private_chars(arguments[i]@)
    }
}

sail_verified! {
    /// Copies exact Unicode scalars; their UTF-8 bytes remain identical.
    pub fn copy_string(value: &str) -> (copy: String)
        requires(true) ensures(copy@ == value@)
    { let mut copy = String::new(); copy.push_str(value); copy }
}

sail_verified! {
    /// Exact bytes, including full lengths, determine an operational token.
    pub fn bytes_equal(left: &[u8], right: &[u8]) -> (equal: bool)
        requires(true) ensures(equal == (left@ == right@))
    {
        if left.len() != right.len() { return false; }
        let mut i = 0;
        sail_while! { (i < left.len()) invariant(
            i <= left.len(), left.len() == right.len(), forall|j: int| 0 <= j < i ==> left@[j] == right@[j]
        ) decreases(left.len() - i) {
            if left[i] != right[i] { return false; } i += 1;
        }}
        sail_proof! { assert(left@ =~= right@); }
        true
    }
}

sail_verified! {
    /// Matches exact option spelling or that spelling followed by '='.
    pub fn argument_flag(argument: &str, flag: &[u8]) -> (matches: bool)
        requires(true) ensures(matches == flag_bytes(string_bytes(argument), flag@))
    {
        let bytes = argument.as_bytes();
        if bytes.len() < flag.len() { return false; }
        if !bytes_equal(&bytes[..flag.len()], flag) { return false; }
        bytes.len() == flag.len() || bytes[flag.len()] == b'='
    }
}

sail_verified! {
    /// Detects every reserved private spelling before proposing a new envelope.
    pub fn private_argument(argument: &str) -> (reserved: bool)
        requires(true) ensures(reserved == private_option(argument))
    {
        argument_flag(argument, "--sail-observation-request".as_bytes())
            || argument_flag(argument, "--sail-observation-dir".as_bytes())
            || argument_flag(argument, "--sail-observation-envelope-v1".as_bytes())
    }
}

sail_verified! {
    /// Valid UTF-8 paths cannot be empty, option-looking or contain a NUL.
    pub fn path_argument(path: &str) -> (usable: bool)
        requires(true) ensures(usable == usable_path(path))
    {
        let bytes = path.as_bytes();
        if bytes.is_empty() || bytes[0] == b'-' { return false; }
        let mut i = 0;
        sail_while! { (i < bytes.len()) invariant(
            i <= bytes.len(), bytes@ == string_bytes(path), bytes.len() > 0, bytes[0] != 45u8,
            forall|j: int| 0 <= j < i ==> bytes@[j] != 0u8
        ) decreases(bytes.len() - i) {
            if bytes[i] == 0 { return false; } i += 1;
        }}
        true
    }
}

sail_code! {
    /// Operational capture data, never solver-origin authority.
    pub enum ObservationSettings {
        /// An owned escaping layer with no journal authority.
        Off,
        /// Capture paths are used only after runtime/preparation validation.
        On {
            /// Exact independently prepared request path.
            request: String,
            /// Exact independently prepared fresh journal directory.
            directory: String,
        },
    }
    /// Immutable final argv and the exact owned setting actually encoded.
    pub struct ObservationArguments { arguments: Vec<String>, settings: ObservationSettings }
    /// One consumed owned layer and its exact original ordinary argv.
    pub struct ObservationEnvelope { original: Vec<String>, settings: ObservationSettings }
}

sail_spec! {
    pub open spec fn settings_view(settings: ObservationSettings) -> Option<(Seq<char>, Seq<char>)> {
        match settings { ObservationSettings::Off => None,
            ObservationSettings::On { request, directory } => Some((request@, directory@)) }
    }
    pub open spec fn selected_settings(arguments: Seq<String>, requested: ObservationSettings) -> Option<(Seq<char>, Seq<char>)> {
        match requested { ObservationSettings::Off => None,
            ObservationSettings::On { request, directory } =>
                if arguments.len() <= usize::MAX - 4 && usable_chars(request@) && usable_chars(directory@)
                    && forall|i: int| 0 <= i < arguments.len() ==> !private_chars(arguments[i]@) { Some((request@, directory@)) } else { None } }
    }
    pub open spec fn owned_prefix(settings: Option<(Seq<char>, Seq<char>)>) -> Seq<Seq<char>> {
        match settings {
            None => seq!["--sail-observation-envelope-v1"@, "off"@],
            Some(paths) => seq!["--sail-observation-envelope-v1"@, "on"@, paths.0, paths.1],
        }
    }
    pub open spec fn envelope_valid(arguments: Seq<String>) -> bool {
        arguments.len() >= 2 && arguments[0]@ == "--sail-observation-envelope-v1"@
            && (arguments[1]@ == "off"@ || (arguments[1]@ == "on"@ && arguments.len() >= 4
                && usable_chars(arguments[2]@) && usable_chars(arguments[3]@)))
    }
    pub open spec fn envelope_settings(arguments: Seq<String>) -> Option<(Seq<char>, Seq<char>)> {
        if arguments[1]@ == "on"@ { Some((arguments[2]@, arguments[3]@)) } else { None }
    }
}

sail_code! {
    impl ObservationArguments {
        sail_impl_spec! {
            pub closed spec fn arguments_view(&self) -> Seq<Seq<char>> { strings(self.arguments@) }
            pub closed spec fn settings_view(&self) -> Option<(Seq<char>, Seq<char>)> { settings_view(self.settings) }
        }
        sail_method! {
            /// Exact final argv to use before native command configuration/sealing.
            pub fn arguments(&self) -> (arguments: &[String])
                requires(true) ensures(strings(arguments@) == self.arguments_view())
            { &self.arguments }
        }
        sail_method! {
            /// Actual encoded setting; a refusal is explicitly Off, not unchanged argv.
            pub fn settings(&self) -> (settings: &ObservationSettings)
                requires(true) ensures(settings_view(*settings) == self.settings_view())
            { &self.settings }
        }
    }
    impl ObservationEnvelope {
        sail_impl_spec! {
            pub closed spec fn original_view(&self) -> Seq<Seq<char>> { strings(self.original@) }
            pub closed spec fn settings_view(&self) -> Option<(Seq<char>, Seq<char>)> { settings_view(self.settings) }
        }
        sail_method! {
            /// Consumes exactly one owned layer. Original reserved tokens are not parsed again.
            pub fn into_parts(self) -> (parts: (Vec<String>, ObservationSettings))
                requires(true) ensures(strings(parts.0@) == self.original_view(), settings_view(parts.1) == self.settings_view())
            { (self.original, self.settings) }
        }
    }
}

sail_verified! {
    /// Refuses observation for duplicate private tokens; the escaping layer still remains.
    pub fn observation_allowed(arguments: &[String], requested: &ObservationSettings) -> (allowed: bool)
        requires(true)
        ensures(allowed == selected_settings(arguments@, *requested).is_some())
    {
        let (request, directory) = match requested {
            ObservationSettings::Off => return false,
            ObservationSettings::On { request, directory } => (request, directory),
        };
        if !path_argument(request.as_str()) || !path_argument(directory.as_str()) || arguments.len() > usize::MAX - 4 { return false; }
        let mut i = 0;
        sail_while! { (i < arguments.len()) invariant(
            i <= arguments.len(), forall|j: int| 0 <= j < i ==> !private_chars(arguments@[j]@)
        ) decreases(arguments.len() - i) {
            if private_argument(arguments[i].as_str()) { return false; } i += 1;
        }}
        true
    }
}

sail_verified! {
    /// Always wraps one owned layer, including observation refusal/IO failure.
    /// All original tokens are preserved behind it, including valid/nested reserved prefixes.
    pub fn plan_observation_arguments(arguments: &[String], requested: ObservationSettings)
        -> (plan: ObservationArguments)
        requires(true)
        ensures(plan.settings_view() == selected_settings(arguments@, requested),
            plan.arguments_view() == owned_prefix(plan.settings_view()) + strings(arguments@))
    {
        let enabled = observation_allowed(arguments, &requested);
        let mut planned = vec![copy_string("--sail-observation-envelope-v1")];
        let settings = match requested {
            ObservationSettings::On { request, directory } if enabled => {
                planned.push(copy_string("on"));
                planned.push(copy_string(request.as_str())); planned.push(copy_string(directory.as_str()));
                ObservationSettings::On { request, directory }
            },
            _ => { planned.push(copy_string("off")); ObservationSettings::Off },
        };
        sail_proof! { assert(strings(planned@) =~= owned_prefix(settings_view(settings))); }
        let _prefix_len = planned.len(); let mut i = 0;
        sail_while! { (i < arguments.len()) invariant(
            i <= arguments.len(), 2 <= _prefix_len <= 4,
            planned@.len() == i as int + _prefix_len as int,
            strings(planned@.take(_prefix_len as int)) == owned_prefix(settings_view(settings)),
            forall|j: int| 0 <= j < i ==> planned@[j + _prefix_len]@ == #[trigger] arguments@[j]@
        ) decreases(arguments.len() - i) {
            planned.push(copy_string(arguments[i].as_str())); i += 1;
        }}
        sail_proof! {
            let expected = owned_prefix(settings_view(settings)) + strings(arguments@);
            assert forall|j: int| 0 <= j < planned.len() implies #[trigger] planned@[j]@ == expected[j] by {
                if j >= _prefix_len { assert(planned@[j]@ == arguments@[j - _prefix_len]@); }
                else { assert(strings(planned@.take(_prefix_len as int))[j] == planned@[j]@); }
            }
            assert(strings(planned@) =~= expected);
        }
        ObservationArguments { arguments: planned, settings }
    }
}

sail_verified! {
    /// Consumes exactly one complete owned layer. It never recursively interprets
    /// reserved prefixes in restored originals; malformed prefixes stay untouched.
    pub fn strip_observation_arguments(arguments: &[String]) -> (envelope: Option<ObservationEnvelope>)
        requires(true)
        ensures(envelope.is_some() == envelope_valid(arguments@),
            envelope.is_some() ==> envelope.unwrap().settings_view() == envelope_settings(arguments@)
                && envelope.unwrap().original_view() == strings(arguments@).skip(owned_prefix(envelope.unwrap().settings_view()).len() as int))
    {
        if arguments.len() < 2 || arguments[0].as_str() != "--sail-observation-envelope-v1" { return None; }
        let (start, settings) = if arguments[1].as_str() == "off" {
            (2usize, ObservationSettings::Off)
        } else if arguments[1].as_str() == "on" {
            if arguments.len() < 4 || !path_argument(arguments[2].as_str()) || !path_argument(arguments[3].as_str()) { return None; }
            (4usize, ObservationSettings::On { request: copy_string(arguments[2].as_str()), directory: copy_string(arguments[3].as_str()) })
        } else { return None; };
        let mut original: Vec<String> = Vec::new(); let mut i = start;
        sail_while! { (i < arguments.len()) invariant(
            start <= i <= arguments.len(), envelope_valid(arguments@),
            settings_view(settings) == envelope_settings(arguments@), start == owned_prefix(settings_view(settings)).len(),
            original@.len() == i as int - start as int,
            forall|j: int| 0 <= j < i - start ==> #[trigger] original@[j]@ == arguments@[j + start]@
        ) decreases(arguments.len() - i) {
            original.push(copy_string(arguments[i].as_str())); i += 1;
        }}
        sail_proof! { assert(strings(original@) =~= strings(arguments@).skip(start as int)); }
        Some(ObservationEnvelope { original, settings })
    }
}

sail_verified! {
    /// Every original argv is restored exactly, including reserved/nested prefixes
    /// and observation refusals. One actual layer is consumed, never recursively.
    pub fn restored_observation_arguments(arguments: &[String], requested: ObservationSettings)
        -> (restored: (Vec<String>, ObservationSettings))
        requires(true)
        ensures(strings(restored.0@) == strings(arguments@),
            settings_view(restored.1) == selected_settings(arguments@, requested))
    {
        sail_proof! { reveal(ObservationArguments::arguments_view); }
        let plan = plan_observation_arguments(arguments, requested);
        let actual = plan.arguments();
        sail_proof! {
            let settings = plan.settings_view();
            let prefix = owned_prefix(settings);
            assert(strings(actual@) == prefix + strings(arguments@));
            assert(actual.len() >= 2);
            assert(strings(actual@)[0] == actual@[0]@);
            assert(strings(actual@)[1] == actual@[1]@);
            assert(actual@[0]@ == "--sail-observation-envelope-v1"@);
            if settings.is_some() {
                assert(actual.len() >= 4);
                assert(strings(actual@)[2] == actual@[2]@);
                assert(strings(actual@)[3] == actual@[3]@);
                assert(actual@[1]@ == "on"@);
                assert(actual@[2]@ == settings.unwrap().0);
                assert(actual@[3]@ == settings.unwrap().1);
            } else { assert(actual@[1]@ == "off"@); }
            assert(envelope_valid(actual@));
            assert(envelope_settings(actual@) == settings);
        }
        match strip_observation_arguments(actual) {
            Some(parsed) => parsed.into_parts(),
            None => (Vec::new(), ObservationSettings::Off),
        }
    }
}

sail_verified! {
    /// Concrete nonempty nested-prefix refusal still restores all original tokens.
    pub fn nested_refusal_roundtrip_witness() -> (passed: bool)
        requires(true) ensures(passed)
    {
        let original = vec![copy_string("--sail-observation-envelope-v1"), copy_string("off"), copy_string("--help")];
        let restored = restored_observation_arguments(&original, ObservationSettings::Off);
        sail_proof! { assert(strings(original@).len() == original.len());
            assert(strings(restored.0@).len() == restored.0.len());
            assert(settings_view(restored.1).is_none()); }
        restored.0.len() == 3 && matches!(restored.1, ObservationSettings::Off)
    }
}

sail_verified! {
    /// Copies every ordinary token without interpreting a second envelope.
    pub fn copy_arguments(arguments: &[String]) -> (copy: Vec<String>)
        requires(true) ensures(strings(copy@) == strings(arguments@))
    {
        let mut copy: Vec<String> = Vec::new(); let mut i = 0;
        sail_while! { (i < arguments.len()) invariant(
            i <= arguments.len(), copy.len() == i,
            forall|j: int| 0 <= j < i ==> #[trigger] copy@[j]@ == arguments@[j]@
        ) decreases(arguments.len() - i) {
            copy.push(copy_string(arguments[i].as_str())); i += 1;
        }}
        sail_proof! { assert(strings(copy@) =~= strings(arguments@)); }
        copy
    }
}

sail_verified! {
    /// Preserves the executable's original argv[0] before its restored ordinary tokens.
    pub fn process_arguments(program: &str, arguments: &[String]) -> (restored: Vec<String>)
        requires(true) ensures(strings(restored@) == seq![program@] + strings(arguments@))
    {
        let mut restored = vec![copy_string(program)]; let mut i = 0;
        sail_while! { (i < arguments.len()) invariant(
            i <= arguments.len(), restored@.len() == i as int + 1, restored@[0]@ == program@,
            forall|j: int| 0 <= j < i ==> restored@[j + 1]@ == #[trigger] arguments@[j]@
        ) decreases(arguments.len() - i) {
            restored.push(copy_string(arguments[i].as_str())); i += 1;
        }}
        sail_proof! {
            let expected = seq![program@] + strings(arguments@);
            assert forall|j: int| 0 <= j < restored.len() implies #[trigger] restored@[j]@ == expected[j] by {
                if j > 0 { assert(restored@[j]@ == arguments@[j - 1]@); }
            }
            assert(strings(restored@) =~= expected);
        }
        restored
    }
}
