// auto-generated: "lalrpop 0.22.0"
// sha3: 536ff210e9961da06997d261c2c564d4019c4e9501e486b50da30184cf7f19e3
use crate::lexer::{self, Token};
use crate::{Block, Data, Dictionary, Statement};
#[allow(unused_extern_crates)]
extern crate lalrpop_util as __lalrpop_util;
#[allow(unused_imports)]
use self::__lalrpop_util::state_machine as __state_machine;
#[allow(unused_extern_crates)]
extern crate alloc;

#[rustfmt::skip]
#[allow(explicit_outlives_requirements, non_snake_case, non_camel_case_types, unused_mut, unused_variables, unused_imports, unused_parens, clippy::needless_lifetimes, clippy::type_complexity, clippy::needless_return, clippy::too_many_arguments, clippy::never_loop, clippy::match_single_binding, clippy::needless_raw_string_hashes)]
mod __parse__Block {

    use crate::lexer::{self, Token};
    use crate::{Block, Data, Dictionary, Statement};
    #[allow(unused_extern_crates)]
    extern crate lalrpop_util as __lalrpop_util;
    #[allow(unused_imports)]
    use self::__lalrpop_util::state_machine as __state_machine;
    #[allow(unused_extern_crates)]
    extern crate alloc;
    use super::__ToTriple;
    #[allow(dead_code)]
    pub(crate) enum __Symbol<>
     {
        Variant0(lexer::Token),
        Variant1(Block),
        Variant2(Data),
        Variant3(Dictionary),
        Variant4(Statement),
        Variant5(alloc::vec::Vec<Statement>),
        Variant6(Vec<Statement>),
        Variant7(Option<lexer::Token>),
    }
    const __ACTION: &[i8] = &[
        // State 0
        0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0,
        // State 1
        0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 4, 0, 0, 0,
        // State 2
        0, 0, 0, 0, 15, 0, 16, 0, 0, 6, 18, 19, 17, 0,
        // State 3
        0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 4
        0, 0, 0, 0, 15, 0, 22, 0, 0, 6, 18, 19, 17, 0,
        // State 5
        24, 0, 9, 8, 0, 3, 0, 0, 0, 0, 4, 0, 0, 0,
        // State 6
        0, 0, 0, 0, 15, 3, 0, 0, 0, 0, 18, 19, 17, 0,
        // State 7
        0, 0, 0, 0, 15, 3, 0, 0, 0, 0, 18, 19, 17, 0,
        // State 8
        0, 0, 0, 0, 15, 0, 0, 0, 0, 0, 18, 19, 17, 0,
        // State 9
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 10
        0, 0, 0, 0, -2, 0, -2, 0, 0, -2, -2, -2, -2, 0,
        // State 11
        0, 0, 0, 0, -10, 0, -10, 0, 0, -10, -10, -10, -10, 0,
        // State 12
        0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 13
        0, 0, 0, 0, -18, 0, -18, 0, 0, -18, -18, -18, -18, 0,
        // State 14
        0, 0, 0, 0, 0, 0, 0, 0, 0, 23, 0, 0, 0, 0,
        // State 15
        0, 0, 0, 0, -7, 0, -7, 0, 0, -7, -7, -7, -7, 0,
        // State 16
        -5, 0, 0, -5, -5, 0, -5, 0, 0, -5, -5, -5, -5, 0,
        // State 17
        -3, 0, 0, -3, -3, 0, -3, 0, 0, -3, -3, -3, -3, 0,
        // State 18
        -4, 0, 0, -4, -4, 0, -4, 0, 0, -4, -4, -4, -4, 0,
        // State 19
        0, 0, 0, 0, -1, 0, -1, 0, 0, -1, -1, -1, -1, 0,
        // State 20
        0, 0, 0, 0, -19, 0, -19, 0, 0, -19, -19, -19, -19, 0,
        // State 21
        0, 0, 0, 0, -8, 0, -8, 0, 0, -8, -8, -8, -8, 0,
        // State 22
        -6, 0, 0, -6, -6, 0, -6, 0, 0, -6, -6, -6, -6, 0,
        // State 23
        0, 0, 0, 0, -15, 0, -15, 0, 0, -15, -15, -15, -15, 0,
        // State 24
        30, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 25
        0, 0, 0, 0, -14, 0, -14, 0, 0, -14, -14, -14, -14, 0,
        // State 26
        31, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 27
        0, 0, 0, 0, -13, 0, -13, 0, 0, -13, -13, -13, -13, 0,
        // State 28
        0, 0, 0, 0, -9, 0, -9, 0, 0, -9, -9, -9, -9, 0,
        // State 29
        0, 0, 0, 0, -12, 0, -12, 0, 0, -12, -12, -12, -12, 0,
        // State 30
        0, 0, 0, 0, -11, 0, -11, 0, 0, -11, -11, -11, -11, 0,
    ];
    fn __action(state: i8, integer: usize) -> i8 {
        __ACTION[(state as usize) * 14 + integer]
    }
    const __EOF_ACTION: &[i8] = &[
        // State 0
        0,
        // State 1
        0,
        // State 2
        0,
        // State 3
        0,
        // State 4
        0,
        // State 5
        0,
        // State 6
        0,
        // State 7
        0,
        // State 8
        0,
        // State 9
        -22,
        // State 10
        -2,
        // State 11
        0,
        // State 12
        0,
        // State 13
        0,
        // State 14
        0,
        // State 15
        -7,
        // State 16
        0,
        // State 17
        0,
        // State 18
        0,
        // State 19
        -1,
        // State 20
        0,
        // State 21
        -8,
        // State 22
        0,
        // State 23
        0,
        // State 24
        0,
        // State 25
        0,
        // State 26
        0,
        // State 27
        0,
        // State 28
        0,
        // State 29
        0,
        // State 30
        0,
    ];
    fn __goto(state: i8, nt: usize) -> i8 {
        match nt {
            0 => match state {
                0 => 9,
                _ => 11,
            },
            1 => match state {
                6 => 24,
                7 => 26,
                8 => 28,
                _ => 12,
            },
            2 => match state {
                3 => 19,
                6 => 25,
                7 => 27,
                _ => 10,
            },
            3 => match state {
                4 => 20,
                _ => 13,
            },
            5 => 4,
            _ => 0,
        }
    }
    const __TERMINAL: &[&str] = &[
        r###"",""###,
        r###""%""###,
        r###""=""###,
        r###"":""###,
        r###""$""###,
        r###""{""###,
        r###""}""###,
        r###""[""###,
        r###""]""###,
        r###"atom"###,
        r###"sqs"###,
        r###"tqs"###,
        r###"f64"###,
        r###"comment"###,
    ];
    fn __expected_tokens(__state: i8) -> alloc::vec::Vec<alloc::string::String> {
        __TERMINAL.iter().enumerate().filter_map(|(index, terminal)| {
            let next_state = __action(__state, index);
            if next_state == 0 {
                None
            } else {
                Some(alloc::string::ToString::to_string(terminal))
            }
        }).collect()
    }
    fn __expected_tokens_from_states<
    >(
        __states: &[i8],
        _: core::marker::PhantomData<()>,
    ) -> alloc::vec::Vec<alloc::string::String>
    {
        __TERMINAL.iter().enumerate().filter_map(|(index, terminal)| {
            if __accepts(None, __states, Some(index), core::marker::PhantomData::<()>) {
                Some(alloc::string::ToString::to_string(terminal))
            } else {
                None
            }
        }).collect()
    }
    struct __StateMachine<>
    where 
    {
        __phantom: core::marker::PhantomData<()>,
    }
    impl<> __state_machine::ParserDefinition for __StateMachine<>
    where 
    {
        type Location = lexer::Location;
        type Error = lexer::LexicalError;
        type Token = lexer::Token;
        type TokenIndex = usize;
        type Symbol = __Symbol<>;
        type Success = Block;
        type StateIndex = i8;
        type Action = i8;
        type ReduceIndex = i8;
        type NonterminalIndex = usize;

        #[inline]
        fn start_location(&self) -> Self::Location {
              Default::default()
        }

        #[inline]
        fn start_state(&self) -> Self::StateIndex {
              0
        }

        #[inline]
        fn token_to_index(&self, token: &Self::Token) -> Option<usize> {
            __token_to_integer(token, core::marker::PhantomData::<()>)
        }

        #[inline]
        fn action(&self, state: i8, integer: usize) -> i8 {
            __action(state, integer)
        }

        #[inline]
        fn error_action(&self, state: i8) -> i8 {
            __action(state, 14 - 1)
        }

        #[inline]
        fn eof_action(&self, state: i8) -> i8 {
            __EOF_ACTION[state as usize]
        }

        #[inline]
        fn goto(&self, state: i8, nt: usize) -> i8 {
            __goto(state, nt)
        }

        fn token_to_symbol(&self, token_index: usize, token: Self::Token) -> Self::Symbol {
            __token_to_symbol(token_index, token, core::marker::PhantomData::<()>)
        }

        fn expected_tokens(&self, state: i8) -> alloc::vec::Vec<alloc::string::String> {
            __expected_tokens(state)
        }

        fn expected_tokens_from_states(&self, states: &[i8]) -> alloc::vec::Vec<alloc::string::String> {
            __expected_tokens_from_states(states, core::marker::PhantomData::<()>)
        }

        #[inline]
        fn uses_error_recovery(&self) -> bool {
            false
        }

        #[inline]
        fn error_recovery_symbol(
            &self,
            recovery: __state_machine::ErrorRecovery<Self>,
        ) -> Self::Symbol {
            panic!("error recovery not enabled for this grammar")
        }

        fn reduce(
            &mut self,
            action: i8,
            start_location: Option<&Self::Location>,
            states: &mut alloc::vec::Vec<i8>,
            symbols: &mut alloc::vec::Vec<__state_machine::SymbolTriple<Self>>,
        ) -> Option<__state_machine::ParseResult<Self>> {
            __reduce(
                action,
                start_location,
                states,
                symbols,
                core::marker::PhantomData::<()>,
            )
        }

        fn simulate_reduce(&self, action: i8) -> __state_machine::SimulatedReduce<Self> {
            __simulate_reduce(action, core::marker::PhantomData::<()>)
        }
    }
    fn __token_to_integer<
    >(
        __token: &lexer::Token,
        _: core::marker::PhantomData<()>,
    ) -> Option<usize>
    {
        #[warn(unused_variables)]
        match __token {
            Token::Comma if true => Some(0),
            Token::Percent if true => Some(1),
            Token::Equals if true => Some(2),
            Token::Colon if true => Some(3),
            Token::DollarSign if true => Some(4),
            Token::LeftBrace if true => Some(5),
            Token::RightBrace if true => Some(6),
            Token::LeftBracket if true => Some(7),
            Token::RightBracket if true => Some(8),
            Token::Atom(_) if true => Some(9),
            Token::SingleQuotedString(_) if true => Some(10),
            Token::TripleQuotedString(_) if true => Some(11),
            Token::F64(_) if true => Some(12),
            Token::Comment(_) if true => Some(13),
            _ => None,
        }
    }
    fn __token_to_symbol<
    >(
        __token_index: usize,
        __token: lexer::Token,
        _: core::marker::PhantomData<()>,
    ) -> __Symbol<>
    {
        #[allow(clippy::manual_range_patterns)]match __token_index {
            0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 => __Symbol::Variant0(__token),
            _ => unreachable!(),
        }
    }
    fn __simulate_reduce<
    >(
        __reduce_index: i8,
        _: core::marker::PhantomData<()>,
    ) -> __state_machine::SimulatedReduce<__StateMachine<>>
    {
        match __reduce_index {
            0 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 0,
                }
            }
            1 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 0,
                }
            }
            2 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 1,
                }
            }
            3 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 1,
                }
            }
            4 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 1,
                }
            }
            5 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 1,
                }
            }
            6 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 2,
                }
            }
            7 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 2,
                }
            }
            8 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 3,
                }
            }
            9 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 3,
                }
            }
            10 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 4,
                    nonterminal_produced: 3,
                }
            }
            11 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 4,
                    nonterminal_produced: 3,
                }
            }
            12 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 3,
                }
            }
            13 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 3,
                }
            }
            14 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 3,
                }
            }
            15 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 4,
                }
            }
            16 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 4,
                }
            }
            17 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 5,
                }
            }
            18 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 5,
                }
            }
            19 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 6,
                }
            }
            20 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 6,
                }
            }
            21 => __state_machine::SimulatedReduce::Accept,
            22 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 8,
                }
            }
            23 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 9,
                }
            }
            24 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 10,
                }
            }
            25 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 11,
                }
            }
            26 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 12,
                }
            }
            27 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 12,
                }
            }
            _ => panic!("invalid reduction index {}", __reduce_index)
        }
    }
    pub struct BlockParser {
        _priv: (),
    }

    impl Default for BlockParser { fn default() -> Self { Self::new() } }
    impl BlockParser {
        pub fn new() -> BlockParser {
            BlockParser {
                _priv: (),
            }
        }

        #[allow(dead_code)]
        pub fn parse<
            __TOKEN: __ToTriple<>,
            __TOKENS: IntoIterator<Item=__TOKEN>,
        >(
            &self,
            __tokens0: __TOKENS,
        ) -> Result<Block, __lalrpop_util::ParseError<lexer::Location, lexer::Token, lexer::LexicalError>>
        {
            let __tokens = __tokens0.into_iter();
            let mut __tokens = __tokens.map(|t| __ToTriple::to_triple(t));
            __state_machine::Parser::drive(
                __StateMachine {
                    __phantom: core::marker::PhantomData::<()>,
                },
                __tokens,
            )
        }
    }
    fn __accepts<
    >(
        __error_state: Option<i8>,
        __states: &[i8],
        __opt_integer: Option<usize>,
        _: core::marker::PhantomData<()>,
    ) -> bool
    {
        let mut __states = __states.to_vec();
        __states.extend(__error_state);
        loop {
            let mut __states_len = __states.len();
            let __top = __states[__states_len - 1];
            let __action = match __opt_integer {
                None => __EOF_ACTION[__top as usize],
                Some(__integer) => __action(__top, __integer),
            };
            if __action == 0 { return false; }
            if __action > 0 { return true; }
            let (__to_pop, __nt) = match __simulate_reduce(-(__action + 1), core::marker::PhantomData::<()>) {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop, nonterminal_produced
                } => (states_to_pop, nonterminal_produced),
                __state_machine::SimulatedReduce::Accept => return true,
            };
            __states_len -= __to_pop;
            __states.truncate(__states_len);
            let __top = __states[__states_len - 1];
            let __next_state = __goto(__top, __nt);
            __states.push(__next_state);
        }
    }
    fn __reduce<
    >(
        __action: i8,
        __lookahead_start: Option<&lexer::Location>,
        __states: &mut alloc::vec::Vec<i8>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> Option<Result<Block,__lalrpop_util::ParseError<lexer::Location, lexer::Token, lexer::LexicalError>>>
    {
        let (__pop_states, __nonterminal) = match __action {
            0 => {
                __reduce0(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            1 => {
                __reduce1(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            2 => {
                __reduce2(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            3 => {
                __reduce3(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            4 => {
                __reduce4(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            5 => {
                __reduce5(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            6 => {
                __reduce6(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            7 => {
                __reduce7(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            8 => {
                __reduce8(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            9 => {
                __reduce9(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            10 => {
                __reduce10(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            11 => {
                __reduce11(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            12 => {
                __reduce12(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            13 => {
                __reduce13(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            14 => {
                __reduce14(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            15 => {
                __reduce15(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            16 => {
                __reduce16(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            17 => {
                __reduce17(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            18 => {
                __reduce18(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            19 => {
                __reduce19(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            20 => {
                __reduce20(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            21 => {
                // __Block = Block => ActionFn(2);
                let __sym0 = __pop_Variant1(__symbols);
                let __start = __sym0.0;
                let __end = __sym0.2;
                let __nt = super::__action2::<>(__sym0);
                return Some(Ok(__nt));
            }
            22 => {
                __reduce22(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            23 => {
                __reduce23(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            24 => {
                __reduce24(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            25 => {
                __reduce25(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            26 => {
                __reduce26(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            27 => {
                __reduce27(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            _ => panic!("invalid action code {}", __action)
        };
        let __states_len = __states.len();
        __states.truncate(__states_len - __pop_states);
        let __state = *__states.last().unwrap();
        let __next_state = __goto(__state, __nonterminal);
        __states.push(__next_state);
        None
    }
    #[inline(never)]
    fn __symbol_type_mismatch() -> ! {
        panic!("symbol type mismatch")
    }
    fn __pop_Variant1<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Block, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant1(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant2<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Data, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant2(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant3<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Dictionary, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant3(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant7<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Option<lexer::Token>, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant7(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant4<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Statement, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant4(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant6<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Vec<Statement>, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant6(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant5<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, alloc::vec::Vec<Statement>, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant5(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant0<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, lexer::Token, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant0(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __reduce0<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Block = atom, sqs, Dictionary => ActionFn(29);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant3(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action29::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (3, 0)
    }
    fn __reduce1<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Block = atom, Dictionary => ActionFn(30);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant3(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action30::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (2, 0)
    }
    fn __reduce2<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Data = sqs => ActionFn(15);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action15::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (1, 1)
    }
    fn __reduce3<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Data = tqs => ActionFn(16);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action16::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (1, 1)
    }
    fn __reduce4<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Data = f64 => ActionFn(17);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action17::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (1, 1)
    }
    fn __reduce5<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Data = "$", atom => ActionFn(18);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action18::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (2, 1)
    }
    fn __reduce6<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Dictionary = "{", "}" => ActionFn(25);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action25::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant3(__nt), __end));
        (2, 2)
    }
    fn __reduce7<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Dictionary = "{", Statement+, "}" => ActionFn(26);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant0(__symbols);
        let __sym1 = __pop_Variant5(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action26::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant3(__nt), __end));
        (3, 2)
    }
    fn __reduce8<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = atom, "=", Data => ActionFn(6);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant2(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action6::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (3, 3)
    }
    fn __reduce9<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = Block => ActionFn(7);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action7::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (1, 3)
    }
    fn __reduce10<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = atom, ":", Data, "," => ActionFn(8);
        assert!(__symbols.len() >= 4);
        let __sym3 = __pop_Variant0(__symbols);
        let __sym2 = __pop_Variant2(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym3.2;
        let __nt = super::__action8::<>(__sym0, __sym1, __sym2, __sym3);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (4, 3)
    }
    fn __reduce11<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = Data, ":", Data, "," => ActionFn(9);
        assert!(__symbols.len() >= 4);
        let __sym3 = __pop_Variant0(__symbols);
        let __sym2 = __pop_Variant2(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant2(__symbols);
        let __start = __sym0.0;
        let __end = __sym3.2;
        let __nt = super::__action9::<>(__sym0, __sym1, __sym2, __sym3);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (4, 3)
    }
    fn __reduce12<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = atom, ":", Dictionary => ActionFn(10);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant3(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action10::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (3, 3)
    }
    fn __reduce13<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = Data, ":", Dictionary => ActionFn(11);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant3(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant2(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action11::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (3, 3)
    }
    fn __reduce14<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = atom, "," => ActionFn(12);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action12::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (2, 3)
    }
    fn __reduce15<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement* =  => ActionFn(21);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action21::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant5(__nt), __end));
        (0, 4)
    }
    fn __reduce16<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement* = Statement+ => ActionFn(22);
        let __sym0 = __pop_Variant5(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action22::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant5(__nt), __end));
        (1, 4)
    }
    fn __reduce17<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement+ = Statement => ActionFn(23);
        let __sym0 = __pop_Variant4(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action23::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant5(__nt), __end));
        (1, 5)
    }
    fn __reduce18<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement+ = Statement+, Statement => ActionFn(24);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant4(__symbols);
        let __sym0 = __pop_Variant5(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action24::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant5(__nt), __end));
        (2, 5)
    }
    fn __reduce19<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statements =  => ActionFn(27);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action27::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant6(__nt), __end));
        (0, 6)
    }
    fn __reduce20<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statements = Statement+ => ActionFn(28);
        let __sym0 = __pop_Variant5(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action28::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant6(__nt), __end));
        (1, 6)
    }
    fn __reduce22<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // __Data = Data => ActionFn(4);
        let __sym0 = __pop_Variant2(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action4::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (1, 8)
    }
    fn __reduce23<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // __Dictionary = Dictionary => ActionFn(3);
        let __sym0 = __pop_Variant3(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action3::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant3(__nt), __end));
        (1, 9)
    }
    fn __reduce24<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // __Statement = Statement => ActionFn(1);
        let __sym0 = __pop_Variant4(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action1::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (1, 10)
    }
    fn __reduce25<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // __Statements = Statements => ActionFn(0);
        let __sym0 = __pop_Variant6(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action0::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant6(__nt), __end));
        (1, 11)
    }
    fn __reduce26<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // sqs? = sqs => ActionFn(19);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action19::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant7(__nt), __end));
        (1, 12)
    }
    fn __reduce27<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // sqs? =  => ActionFn(20);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action20::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant7(__nt), __end));
        (0, 12)
    }
}
#[allow(unused_imports)]
pub use self::__parse__Block::BlockParser;

#[rustfmt::skip]
#[allow(explicit_outlives_requirements, non_snake_case, non_camel_case_types, unused_mut, unused_variables, unused_imports, unused_parens, clippy::needless_lifetimes, clippy::type_complexity, clippy::needless_return, clippy::too_many_arguments, clippy::never_loop, clippy::match_single_binding, clippy::needless_raw_string_hashes)]
mod __parse__Data {

    use crate::lexer::{self, Token};
    use crate::{Block, Data, Dictionary, Statement};
    #[allow(unused_extern_crates)]
    extern crate lalrpop_util as __lalrpop_util;
    #[allow(unused_imports)]
    use self::__lalrpop_util::state_machine as __state_machine;
    #[allow(unused_extern_crates)]
    extern crate alloc;
    use super::__ToTriple;
    #[allow(dead_code)]
    pub(crate) enum __Symbol<>
     {
        Variant0(lexer::Token),
        Variant1(Block),
        Variant2(Data),
        Variant3(Dictionary),
        Variant4(Statement),
        Variant5(alloc::vec::Vec<Statement>),
        Variant6(Vec<Statement>),
        Variant7(Option<lexer::Token>),
    }
    const __ACTION: &[i8] = &[
        // State 0
        0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 5, 6, 4, 0,
        // State 1
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 2
        0, 0, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0,
        // State 3
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 4
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 5
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 6
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    fn __action(state: i8, integer: usize) -> i8 {
        __ACTION[(state as usize) * 14 + integer]
    }
    const __EOF_ACTION: &[i8] = &[
        // State 0
        0,
        // State 1
        -23,
        // State 2
        0,
        // State 3
        -5,
        // State 4
        -3,
        // State 5
        -4,
        // State 6
        -6,
    ];
    fn __goto(state: i8, nt: usize) -> i8 {
        match nt {
            1 => 1,
            _ => 0,
        }
    }
    const __TERMINAL: &[&str] = &[
        r###"",""###,
        r###""%""###,
        r###""=""###,
        r###"":""###,
        r###""$""###,
        r###""{""###,
        r###""}""###,
        r###""[""###,
        r###""]""###,
        r###"atom"###,
        r###"sqs"###,
        r###"tqs"###,
        r###"f64"###,
        r###"comment"###,
    ];
    fn __expected_tokens(__state: i8) -> alloc::vec::Vec<alloc::string::String> {
        __TERMINAL.iter().enumerate().filter_map(|(index, terminal)| {
            let next_state = __action(__state, index);
            if next_state == 0 {
                None
            } else {
                Some(alloc::string::ToString::to_string(terminal))
            }
        }).collect()
    }
    fn __expected_tokens_from_states<
    >(
        __states: &[i8],
        _: core::marker::PhantomData<()>,
    ) -> alloc::vec::Vec<alloc::string::String>
    {
        __TERMINAL.iter().enumerate().filter_map(|(index, terminal)| {
            if __accepts(None, __states, Some(index), core::marker::PhantomData::<()>) {
                Some(alloc::string::ToString::to_string(terminal))
            } else {
                None
            }
        }).collect()
    }
    struct __StateMachine<>
    where 
    {
        __phantom: core::marker::PhantomData<()>,
    }
    impl<> __state_machine::ParserDefinition for __StateMachine<>
    where 
    {
        type Location = lexer::Location;
        type Error = lexer::LexicalError;
        type Token = lexer::Token;
        type TokenIndex = usize;
        type Symbol = __Symbol<>;
        type Success = Data;
        type StateIndex = i8;
        type Action = i8;
        type ReduceIndex = i8;
        type NonterminalIndex = usize;

        #[inline]
        fn start_location(&self) -> Self::Location {
              Default::default()
        }

        #[inline]
        fn start_state(&self) -> Self::StateIndex {
              0
        }

        #[inline]
        fn token_to_index(&self, token: &Self::Token) -> Option<usize> {
            __token_to_integer(token, core::marker::PhantomData::<()>)
        }

        #[inline]
        fn action(&self, state: i8, integer: usize) -> i8 {
            __action(state, integer)
        }

        #[inline]
        fn error_action(&self, state: i8) -> i8 {
            __action(state, 14 - 1)
        }

        #[inline]
        fn eof_action(&self, state: i8) -> i8 {
            __EOF_ACTION[state as usize]
        }

        #[inline]
        fn goto(&self, state: i8, nt: usize) -> i8 {
            __goto(state, nt)
        }

        fn token_to_symbol(&self, token_index: usize, token: Self::Token) -> Self::Symbol {
            __token_to_symbol(token_index, token, core::marker::PhantomData::<()>)
        }

        fn expected_tokens(&self, state: i8) -> alloc::vec::Vec<alloc::string::String> {
            __expected_tokens(state)
        }

        fn expected_tokens_from_states(&self, states: &[i8]) -> alloc::vec::Vec<alloc::string::String> {
            __expected_tokens_from_states(states, core::marker::PhantomData::<()>)
        }

        #[inline]
        fn uses_error_recovery(&self) -> bool {
            false
        }

        #[inline]
        fn error_recovery_symbol(
            &self,
            recovery: __state_machine::ErrorRecovery<Self>,
        ) -> Self::Symbol {
            panic!("error recovery not enabled for this grammar")
        }

        fn reduce(
            &mut self,
            action: i8,
            start_location: Option<&Self::Location>,
            states: &mut alloc::vec::Vec<i8>,
            symbols: &mut alloc::vec::Vec<__state_machine::SymbolTriple<Self>>,
        ) -> Option<__state_machine::ParseResult<Self>> {
            __reduce(
                action,
                start_location,
                states,
                symbols,
                core::marker::PhantomData::<()>,
            )
        }

        fn simulate_reduce(&self, action: i8) -> __state_machine::SimulatedReduce<Self> {
            __simulate_reduce(action, core::marker::PhantomData::<()>)
        }
    }
    fn __token_to_integer<
    >(
        __token: &lexer::Token,
        _: core::marker::PhantomData<()>,
    ) -> Option<usize>
    {
        #[warn(unused_variables)]
        match __token {
            Token::Comma if true => Some(0),
            Token::Percent if true => Some(1),
            Token::Equals if true => Some(2),
            Token::Colon if true => Some(3),
            Token::DollarSign if true => Some(4),
            Token::LeftBrace if true => Some(5),
            Token::RightBrace if true => Some(6),
            Token::LeftBracket if true => Some(7),
            Token::RightBracket if true => Some(8),
            Token::Atom(_) if true => Some(9),
            Token::SingleQuotedString(_) if true => Some(10),
            Token::TripleQuotedString(_) if true => Some(11),
            Token::F64(_) if true => Some(12),
            Token::Comment(_) if true => Some(13),
            _ => None,
        }
    }
    fn __token_to_symbol<
    >(
        __token_index: usize,
        __token: lexer::Token,
        _: core::marker::PhantomData<()>,
    ) -> __Symbol<>
    {
        #[allow(clippy::manual_range_patterns)]match __token_index {
            0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 => __Symbol::Variant0(__token),
            _ => unreachable!(),
        }
    }
    fn __simulate_reduce<
    >(
        __reduce_index: i8,
        _: core::marker::PhantomData<()>,
    ) -> __state_machine::SimulatedReduce<__StateMachine<>>
    {
        match __reduce_index {
            0 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 0,
                }
            }
            1 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 0,
                }
            }
            2 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 1,
                }
            }
            3 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 1,
                }
            }
            4 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 1,
                }
            }
            5 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 1,
                }
            }
            6 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 2,
                }
            }
            7 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 2,
                }
            }
            8 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 3,
                }
            }
            9 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 3,
                }
            }
            10 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 4,
                    nonterminal_produced: 3,
                }
            }
            11 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 4,
                    nonterminal_produced: 3,
                }
            }
            12 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 3,
                }
            }
            13 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 3,
                }
            }
            14 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 3,
                }
            }
            15 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 4,
                }
            }
            16 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 4,
                }
            }
            17 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 5,
                }
            }
            18 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 5,
                }
            }
            19 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 6,
                }
            }
            20 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 6,
                }
            }
            21 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 7,
                }
            }
            22 => __state_machine::SimulatedReduce::Accept,
            23 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 9,
                }
            }
            24 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 10,
                }
            }
            25 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 11,
                }
            }
            26 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 12,
                }
            }
            27 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 12,
                }
            }
            _ => panic!("invalid reduction index {}", __reduce_index)
        }
    }
    pub struct DataParser {
        _priv: (),
    }

    impl Default for DataParser { fn default() -> Self { Self::new() } }
    impl DataParser {
        pub fn new() -> DataParser {
            DataParser {
                _priv: (),
            }
        }

        #[allow(dead_code)]
        pub fn parse<
            __TOKEN: __ToTriple<>,
            __TOKENS: IntoIterator<Item=__TOKEN>,
        >(
            &self,
            __tokens0: __TOKENS,
        ) -> Result<Data, __lalrpop_util::ParseError<lexer::Location, lexer::Token, lexer::LexicalError>>
        {
            let __tokens = __tokens0.into_iter();
            let mut __tokens = __tokens.map(|t| __ToTriple::to_triple(t));
            __state_machine::Parser::drive(
                __StateMachine {
                    __phantom: core::marker::PhantomData::<()>,
                },
                __tokens,
            )
        }
    }
    fn __accepts<
    >(
        __error_state: Option<i8>,
        __states: &[i8],
        __opt_integer: Option<usize>,
        _: core::marker::PhantomData<()>,
    ) -> bool
    {
        let mut __states = __states.to_vec();
        __states.extend(__error_state);
        loop {
            let mut __states_len = __states.len();
            let __top = __states[__states_len - 1];
            let __action = match __opt_integer {
                None => __EOF_ACTION[__top as usize],
                Some(__integer) => __action(__top, __integer),
            };
            if __action == 0 { return false; }
            if __action > 0 { return true; }
            let (__to_pop, __nt) = match __simulate_reduce(-(__action + 1), core::marker::PhantomData::<()>) {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop, nonterminal_produced
                } => (states_to_pop, nonterminal_produced),
                __state_machine::SimulatedReduce::Accept => return true,
            };
            __states_len -= __to_pop;
            __states.truncate(__states_len);
            let __top = __states[__states_len - 1];
            let __next_state = __goto(__top, __nt);
            __states.push(__next_state);
        }
    }
    fn __reduce<
    >(
        __action: i8,
        __lookahead_start: Option<&lexer::Location>,
        __states: &mut alloc::vec::Vec<i8>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> Option<Result<Data,__lalrpop_util::ParseError<lexer::Location, lexer::Token, lexer::LexicalError>>>
    {
        let (__pop_states, __nonterminal) = match __action {
            0 => {
                __reduce0(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            1 => {
                __reduce1(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            2 => {
                __reduce2(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            3 => {
                __reduce3(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            4 => {
                __reduce4(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            5 => {
                __reduce5(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            6 => {
                __reduce6(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            7 => {
                __reduce7(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            8 => {
                __reduce8(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            9 => {
                __reduce9(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            10 => {
                __reduce10(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            11 => {
                __reduce11(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            12 => {
                __reduce12(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            13 => {
                __reduce13(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            14 => {
                __reduce14(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            15 => {
                __reduce15(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            16 => {
                __reduce16(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            17 => {
                __reduce17(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            18 => {
                __reduce18(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            19 => {
                __reduce19(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            20 => {
                __reduce20(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            21 => {
                __reduce21(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            22 => {
                // __Data = Data => ActionFn(4);
                let __sym0 = __pop_Variant2(__symbols);
                let __start = __sym0.0;
                let __end = __sym0.2;
                let __nt = super::__action4::<>(__sym0);
                return Some(Ok(__nt));
            }
            23 => {
                __reduce23(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            24 => {
                __reduce24(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            25 => {
                __reduce25(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            26 => {
                __reduce26(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            27 => {
                __reduce27(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            _ => panic!("invalid action code {}", __action)
        };
        let __states_len = __states.len();
        __states.truncate(__states_len - __pop_states);
        let __state = *__states.last().unwrap();
        let __next_state = __goto(__state, __nonterminal);
        __states.push(__next_state);
        None
    }
    #[inline(never)]
    fn __symbol_type_mismatch() -> ! {
        panic!("symbol type mismatch")
    }
    fn __pop_Variant1<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Block, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant1(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant2<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Data, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant2(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant3<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Dictionary, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant3(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant7<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Option<lexer::Token>, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant7(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant4<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Statement, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant4(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant6<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Vec<Statement>, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant6(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant5<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, alloc::vec::Vec<Statement>, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant5(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant0<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, lexer::Token, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant0(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __reduce0<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Block = atom, sqs, Dictionary => ActionFn(29);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant3(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action29::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (3, 0)
    }
    fn __reduce1<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Block = atom, Dictionary => ActionFn(30);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant3(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action30::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (2, 0)
    }
    fn __reduce2<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Data = sqs => ActionFn(15);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action15::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (1, 1)
    }
    fn __reduce3<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Data = tqs => ActionFn(16);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action16::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (1, 1)
    }
    fn __reduce4<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Data = f64 => ActionFn(17);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action17::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (1, 1)
    }
    fn __reduce5<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Data = "$", atom => ActionFn(18);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action18::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (2, 1)
    }
    fn __reduce6<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Dictionary = "{", "}" => ActionFn(25);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action25::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant3(__nt), __end));
        (2, 2)
    }
    fn __reduce7<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Dictionary = "{", Statement+, "}" => ActionFn(26);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant0(__symbols);
        let __sym1 = __pop_Variant5(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action26::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant3(__nt), __end));
        (3, 2)
    }
    fn __reduce8<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = atom, "=", Data => ActionFn(6);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant2(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action6::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (3, 3)
    }
    fn __reduce9<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = Block => ActionFn(7);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action7::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (1, 3)
    }
    fn __reduce10<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = atom, ":", Data, "," => ActionFn(8);
        assert!(__symbols.len() >= 4);
        let __sym3 = __pop_Variant0(__symbols);
        let __sym2 = __pop_Variant2(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym3.2;
        let __nt = super::__action8::<>(__sym0, __sym1, __sym2, __sym3);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (4, 3)
    }
    fn __reduce11<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = Data, ":", Data, "," => ActionFn(9);
        assert!(__symbols.len() >= 4);
        let __sym3 = __pop_Variant0(__symbols);
        let __sym2 = __pop_Variant2(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant2(__symbols);
        let __start = __sym0.0;
        let __end = __sym3.2;
        let __nt = super::__action9::<>(__sym0, __sym1, __sym2, __sym3);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (4, 3)
    }
    fn __reduce12<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = atom, ":", Dictionary => ActionFn(10);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant3(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action10::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (3, 3)
    }
    fn __reduce13<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = Data, ":", Dictionary => ActionFn(11);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant3(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant2(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action11::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (3, 3)
    }
    fn __reduce14<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = atom, "," => ActionFn(12);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action12::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (2, 3)
    }
    fn __reduce15<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement* =  => ActionFn(21);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action21::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant5(__nt), __end));
        (0, 4)
    }
    fn __reduce16<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement* = Statement+ => ActionFn(22);
        let __sym0 = __pop_Variant5(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action22::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant5(__nt), __end));
        (1, 4)
    }
    fn __reduce17<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement+ = Statement => ActionFn(23);
        let __sym0 = __pop_Variant4(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action23::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant5(__nt), __end));
        (1, 5)
    }
    fn __reduce18<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement+ = Statement+, Statement => ActionFn(24);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant4(__symbols);
        let __sym0 = __pop_Variant5(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action24::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant5(__nt), __end));
        (2, 5)
    }
    fn __reduce19<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statements =  => ActionFn(27);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action27::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant6(__nt), __end));
        (0, 6)
    }
    fn __reduce20<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statements = Statement+ => ActionFn(28);
        let __sym0 = __pop_Variant5(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action28::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant6(__nt), __end));
        (1, 6)
    }
    fn __reduce21<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // __Block = Block => ActionFn(2);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action2::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (1, 7)
    }
    fn __reduce23<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // __Dictionary = Dictionary => ActionFn(3);
        let __sym0 = __pop_Variant3(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action3::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant3(__nt), __end));
        (1, 9)
    }
    fn __reduce24<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // __Statement = Statement => ActionFn(1);
        let __sym0 = __pop_Variant4(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action1::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (1, 10)
    }
    fn __reduce25<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // __Statements = Statements => ActionFn(0);
        let __sym0 = __pop_Variant6(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action0::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant6(__nt), __end));
        (1, 11)
    }
    fn __reduce26<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // sqs? = sqs => ActionFn(19);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action19::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant7(__nt), __end));
        (1, 12)
    }
    fn __reduce27<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // sqs? =  => ActionFn(20);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action20::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant7(__nt), __end));
        (0, 12)
    }
}
#[allow(unused_imports)]
pub use self::__parse__Data::DataParser;

#[rustfmt::skip]
#[allow(explicit_outlives_requirements, non_snake_case, non_camel_case_types, unused_mut, unused_variables, unused_imports, unused_parens, clippy::needless_lifetimes, clippy::type_complexity, clippy::needless_return, clippy::too_many_arguments, clippy::never_loop, clippy::match_single_binding, clippy::needless_raw_string_hashes)]
mod __parse__Dictionary {

    use crate::lexer::{self, Token};
    use crate::{Block, Data, Dictionary, Statement};
    #[allow(unused_extern_crates)]
    extern crate lalrpop_util as __lalrpop_util;
    #[allow(unused_imports)]
    use self::__lalrpop_util::state_machine as __state_machine;
    #[allow(unused_extern_crates)]
    extern crate alloc;
    use super::__ToTriple;
    #[allow(dead_code)]
    pub(crate) enum __Symbol<>
     {
        Variant0(lexer::Token),
        Variant1(Block),
        Variant2(Data),
        Variant3(Dictionary),
        Variant4(Statement),
        Variant5(alloc::vec::Vec<Statement>),
        Variant6(Vec<Statement>),
        Variant7(Option<lexer::Token>),
    }
    const __ACTION: &[i8] = &[
        // State 0
        0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 1
        0, 0, 0, 0, 13, 0, 14, 0, 0, 4, 16, 17, 15, 0,
        // State 2
        0, 0, 0, 0, 13, 0, 19, 0, 0, 4, 16, 17, 15, 0,
        // State 3
        22, 0, 7, 6, 0, 2, 0, 0, 0, 0, 8, 0, 0, 0,
        // State 4
        0, 0, 0, 0, 13, 2, 0, 0, 0, 0, 16, 17, 15, 0,
        // State 5
        0, 0, 0, 0, 13, 2, 0, 0, 0, 0, 16, 17, 15, 0,
        // State 6
        0, 0, 0, 0, 13, 0, 0, 0, 0, 0, 16, 17, 15, 0,
        // State 7
        0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 8
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 9
        0, 0, 0, 0, -10, 0, -10, 0, 0, -10, -10, -10, -10, 0,
        // State 10
        0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 11
        0, 0, 0, 0, -18, 0, -18, 0, 0, -18, -18, -18, -18, 0,
        // State 12
        0, 0, 0, 0, 0, 0, 0, 0, 0, 20, 0, 0, 0, 0,
        // State 13
        0, 0, 0, 0, -7, 0, -7, 0, 0, -7, -7, -7, -7, 0,
        // State 14
        -5, 0, 0, -5, -5, 0, -5, 0, 0, -5, -5, -5, -5, 0,
        // State 15
        -3, 0, 0, -3, -3, 0, -3, 0, 0, -3, -3, -3, -3, 0,
        // State 16
        -4, 0, 0, -4, -4, 0, -4, 0, 0, -4, -4, -4, -4, 0,
        // State 17
        0, 0, 0, 0, -19, 0, -19, 0, 0, -19, -19, -19, -19, 0,
        // State 18
        0, 0, 0, 0, -8, 0, -8, 0, 0, -8, -8, -8, -8, 0,
        // State 19
        -6, 0, 0, -6, -6, 0, -6, 0, 0, -6, -6, -6, -6, 0,
        // State 20
        0, 0, 0, 0, -2, 0, -2, 0, 0, -2, -2, -2, -2, 0,
        // State 21
        0, 0, 0, 0, -15, 0, -15, 0, 0, -15, -15, -15, -15, 0,
        // State 22
        29, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 23
        0, 0, 0, 0, -14, 0, -14, 0, 0, -14, -14, -14, -14, 0,
        // State 24
        30, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 25
        0, 0, 0, 0, -13, 0, -13, 0, 0, -13, -13, -13, -13, 0,
        // State 26
        0, 0, 0, 0, -9, 0, -9, 0, 0, -9, -9, -9, -9, 0,
        // State 27
        0, 0, 0, 0, -1, 0, -1, 0, 0, -1, -1, -1, -1, 0,
        // State 28
        0, 0, 0, 0, -12, 0, -12, 0, 0, -12, -12, -12, -12, 0,
        // State 29
        0, 0, 0, 0, -11, 0, -11, 0, 0, -11, -11, -11, -11, 0,
    ];
    fn __action(state: i8, integer: usize) -> i8 {
        __ACTION[(state as usize) * 14 + integer]
    }
    const __EOF_ACTION: &[i8] = &[
        // State 0
        0,
        // State 1
        0,
        // State 2
        0,
        // State 3
        0,
        // State 4
        0,
        // State 5
        0,
        // State 6
        0,
        // State 7
        0,
        // State 8
        -24,
        // State 9
        0,
        // State 10
        0,
        // State 11
        0,
        // State 12
        0,
        // State 13
        -7,
        // State 14
        0,
        // State 15
        0,
        // State 16
        0,
        // State 17
        0,
        // State 18
        -8,
        // State 19
        0,
        // State 20
        0,
        // State 21
        0,
        // State 22
        0,
        // State 23
        0,
        // State 24
        0,
        // State 25
        0,
        // State 26
        0,
        // State 27
        0,
        // State 28
        0,
        // State 29
        0,
    ];
    fn __goto(state: i8, nt: usize) -> i8 {
        match nt {
            0 => 9,
            1 => match state {
                4 => 22,
                5 => 24,
                6 => 26,
                _ => 10,
            },
            2 => match state {
                3 => 20,
                4 => 23,
                5 => 25,
                7 => 27,
                _ => 8,
            },
            3 => match state {
                2 => 17,
                _ => 11,
            },
            5 => 2,
            _ => 0,
        }
    }
    const __TERMINAL: &[&str] = &[
        r###"",""###,
        r###""%""###,
        r###""=""###,
        r###"":""###,
        r###""$""###,
        r###""{""###,
        r###""}""###,
        r###""[""###,
        r###""]""###,
        r###"atom"###,
        r###"sqs"###,
        r###"tqs"###,
        r###"f64"###,
        r###"comment"###,
    ];
    fn __expected_tokens(__state: i8) -> alloc::vec::Vec<alloc::string::String> {
        __TERMINAL.iter().enumerate().filter_map(|(index, terminal)| {
            let next_state = __action(__state, index);
            if next_state == 0 {
                None
            } else {
                Some(alloc::string::ToString::to_string(terminal))
            }
        }).collect()
    }
    fn __expected_tokens_from_states<
    >(
        __states: &[i8],
        _: core::marker::PhantomData<()>,
    ) -> alloc::vec::Vec<alloc::string::String>
    {
        __TERMINAL.iter().enumerate().filter_map(|(index, terminal)| {
            if __accepts(None, __states, Some(index), core::marker::PhantomData::<()>) {
                Some(alloc::string::ToString::to_string(terminal))
            } else {
                None
            }
        }).collect()
    }
    struct __StateMachine<>
    where 
    {
        __phantom: core::marker::PhantomData<()>,
    }
    impl<> __state_machine::ParserDefinition for __StateMachine<>
    where 
    {
        type Location = lexer::Location;
        type Error = lexer::LexicalError;
        type Token = lexer::Token;
        type TokenIndex = usize;
        type Symbol = __Symbol<>;
        type Success = Dictionary;
        type StateIndex = i8;
        type Action = i8;
        type ReduceIndex = i8;
        type NonterminalIndex = usize;

        #[inline]
        fn start_location(&self) -> Self::Location {
              Default::default()
        }

        #[inline]
        fn start_state(&self) -> Self::StateIndex {
              0
        }

        #[inline]
        fn token_to_index(&self, token: &Self::Token) -> Option<usize> {
            __token_to_integer(token, core::marker::PhantomData::<()>)
        }

        #[inline]
        fn action(&self, state: i8, integer: usize) -> i8 {
            __action(state, integer)
        }

        #[inline]
        fn error_action(&self, state: i8) -> i8 {
            __action(state, 14 - 1)
        }

        #[inline]
        fn eof_action(&self, state: i8) -> i8 {
            __EOF_ACTION[state as usize]
        }

        #[inline]
        fn goto(&self, state: i8, nt: usize) -> i8 {
            __goto(state, nt)
        }

        fn token_to_symbol(&self, token_index: usize, token: Self::Token) -> Self::Symbol {
            __token_to_symbol(token_index, token, core::marker::PhantomData::<()>)
        }

        fn expected_tokens(&self, state: i8) -> alloc::vec::Vec<alloc::string::String> {
            __expected_tokens(state)
        }

        fn expected_tokens_from_states(&self, states: &[i8]) -> alloc::vec::Vec<alloc::string::String> {
            __expected_tokens_from_states(states, core::marker::PhantomData::<()>)
        }

        #[inline]
        fn uses_error_recovery(&self) -> bool {
            false
        }

        #[inline]
        fn error_recovery_symbol(
            &self,
            recovery: __state_machine::ErrorRecovery<Self>,
        ) -> Self::Symbol {
            panic!("error recovery not enabled for this grammar")
        }

        fn reduce(
            &mut self,
            action: i8,
            start_location: Option<&Self::Location>,
            states: &mut alloc::vec::Vec<i8>,
            symbols: &mut alloc::vec::Vec<__state_machine::SymbolTriple<Self>>,
        ) -> Option<__state_machine::ParseResult<Self>> {
            __reduce(
                action,
                start_location,
                states,
                symbols,
                core::marker::PhantomData::<()>,
            )
        }

        fn simulate_reduce(&self, action: i8) -> __state_machine::SimulatedReduce<Self> {
            __simulate_reduce(action, core::marker::PhantomData::<()>)
        }
    }
    fn __token_to_integer<
    >(
        __token: &lexer::Token,
        _: core::marker::PhantomData<()>,
    ) -> Option<usize>
    {
        #[warn(unused_variables)]
        match __token {
            Token::Comma if true => Some(0),
            Token::Percent if true => Some(1),
            Token::Equals if true => Some(2),
            Token::Colon if true => Some(3),
            Token::DollarSign if true => Some(4),
            Token::LeftBrace if true => Some(5),
            Token::RightBrace if true => Some(6),
            Token::LeftBracket if true => Some(7),
            Token::RightBracket if true => Some(8),
            Token::Atom(_) if true => Some(9),
            Token::SingleQuotedString(_) if true => Some(10),
            Token::TripleQuotedString(_) if true => Some(11),
            Token::F64(_) if true => Some(12),
            Token::Comment(_) if true => Some(13),
            _ => None,
        }
    }
    fn __token_to_symbol<
    >(
        __token_index: usize,
        __token: lexer::Token,
        _: core::marker::PhantomData<()>,
    ) -> __Symbol<>
    {
        #[allow(clippy::manual_range_patterns)]match __token_index {
            0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 => __Symbol::Variant0(__token),
            _ => unreachable!(),
        }
    }
    fn __simulate_reduce<
    >(
        __reduce_index: i8,
        _: core::marker::PhantomData<()>,
    ) -> __state_machine::SimulatedReduce<__StateMachine<>>
    {
        match __reduce_index {
            0 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 0,
                }
            }
            1 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 0,
                }
            }
            2 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 1,
                }
            }
            3 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 1,
                }
            }
            4 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 1,
                }
            }
            5 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 1,
                }
            }
            6 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 2,
                }
            }
            7 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 2,
                }
            }
            8 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 3,
                }
            }
            9 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 3,
                }
            }
            10 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 4,
                    nonterminal_produced: 3,
                }
            }
            11 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 4,
                    nonterminal_produced: 3,
                }
            }
            12 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 3,
                }
            }
            13 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 3,
                }
            }
            14 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 3,
                }
            }
            15 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 4,
                }
            }
            16 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 4,
                }
            }
            17 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 5,
                }
            }
            18 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 5,
                }
            }
            19 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 6,
                }
            }
            20 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 6,
                }
            }
            21 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 7,
                }
            }
            22 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 8,
                }
            }
            23 => __state_machine::SimulatedReduce::Accept,
            24 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 10,
                }
            }
            25 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 11,
                }
            }
            26 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 12,
                }
            }
            27 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 12,
                }
            }
            _ => panic!("invalid reduction index {}", __reduce_index)
        }
    }
    pub struct DictionaryParser {
        _priv: (),
    }

    impl Default for DictionaryParser { fn default() -> Self { Self::new() } }
    impl DictionaryParser {
        pub fn new() -> DictionaryParser {
            DictionaryParser {
                _priv: (),
            }
        }

        #[allow(dead_code)]
        pub fn parse<
            __TOKEN: __ToTriple<>,
            __TOKENS: IntoIterator<Item=__TOKEN>,
        >(
            &self,
            __tokens0: __TOKENS,
        ) -> Result<Dictionary, __lalrpop_util::ParseError<lexer::Location, lexer::Token, lexer::LexicalError>>
        {
            let __tokens = __tokens0.into_iter();
            let mut __tokens = __tokens.map(|t| __ToTriple::to_triple(t));
            __state_machine::Parser::drive(
                __StateMachine {
                    __phantom: core::marker::PhantomData::<()>,
                },
                __tokens,
            )
        }
    }
    fn __accepts<
    >(
        __error_state: Option<i8>,
        __states: &[i8],
        __opt_integer: Option<usize>,
        _: core::marker::PhantomData<()>,
    ) -> bool
    {
        let mut __states = __states.to_vec();
        __states.extend(__error_state);
        loop {
            let mut __states_len = __states.len();
            let __top = __states[__states_len - 1];
            let __action = match __opt_integer {
                None => __EOF_ACTION[__top as usize],
                Some(__integer) => __action(__top, __integer),
            };
            if __action == 0 { return false; }
            if __action > 0 { return true; }
            let (__to_pop, __nt) = match __simulate_reduce(-(__action + 1), core::marker::PhantomData::<()>) {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop, nonterminal_produced
                } => (states_to_pop, nonterminal_produced),
                __state_machine::SimulatedReduce::Accept => return true,
            };
            __states_len -= __to_pop;
            __states.truncate(__states_len);
            let __top = __states[__states_len - 1];
            let __next_state = __goto(__top, __nt);
            __states.push(__next_state);
        }
    }
    fn __reduce<
    >(
        __action: i8,
        __lookahead_start: Option<&lexer::Location>,
        __states: &mut alloc::vec::Vec<i8>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> Option<Result<Dictionary,__lalrpop_util::ParseError<lexer::Location, lexer::Token, lexer::LexicalError>>>
    {
        let (__pop_states, __nonterminal) = match __action {
            0 => {
                __reduce0(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            1 => {
                __reduce1(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            2 => {
                __reduce2(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            3 => {
                __reduce3(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            4 => {
                __reduce4(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            5 => {
                __reduce5(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            6 => {
                __reduce6(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            7 => {
                __reduce7(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            8 => {
                __reduce8(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            9 => {
                __reduce9(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            10 => {
                __reduce10(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            11 => {
                __reduce11(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            12 => {
                __reduce12(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            13 => {
                __reduce13(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            14 => {
                __reduce14(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            15 => {
                __reduce15(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            16 => {
                __reduce16(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            17 => {
                __reduce17(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            18 => {
                __reduce18(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            19 => {
                __reduce19(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            20 => {
                __reduce20(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            21 => {
                __reduce21(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            22 => {
                __reduce22(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            23 => {
                // __Dictionary = Dictionary => ActionFn(3);
                let __sym0 = __pop_Variant3(__symbols);
                let __start = __sym0.0;
                let __end = __sym0.2;
                let __nt = super::__action3::<>(__sym0);
                return Some(Ok(__nt));
            }
            24 => {
                __reduce24(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            25 => {
                __reduce25(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            26 => {
                __reduce26(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            27 => {
                __reduce27(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            _ => panic!("invalid action code {}", __action)
        };
        let __states_len = __states.len();
        __states.truncate(__states_len - __pop_states);
        let __state = *__states.last().unwrap();
        let __next_state = __goto(__state, __nonterminal);
        __states.push(__next_state);
        None
    }
    #[inline(never)]
    fn __symbol_type_mismatch() -> ! {
        panic!("symbol type mismatch")
    }
    fn __pop_Variant1<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Block, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant1(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant2<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Data, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant2(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant3<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Dictionary, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant3(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant7<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Option<lexer::Token>, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant7(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant4<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Statement, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant4(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant6<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Vec<Statement>, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant6(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant5<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, alloc::vec::Vec<Statement>, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant5(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant0<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, lexer::Token, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant0(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __reduce0<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Block = atom, sqs, Dictionary => ActionFn(29);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant3(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action29::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (3, 0)
    }
    fn __reduce1<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Block = atom, Dictionary => ActionFn(30);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant3(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action30::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (2, 0)
    }
    fn __reduce2<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Data = sqs => ActionFn(15);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action15::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (1, 1)
    }
    fn __reduce3<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Data = tqs => ActionFn(16);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action16::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (1, 1)
    }
    fn __reduce4<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Data = f64 => ActionFn(17);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action17::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (1, 1)
    }
    fn __reduce5<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Data = "$", atom => ActionFn(18);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action18::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (2, 1)
    }
    fn __reduce6<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Dictionary = "{", "}" => ActionFn(25);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action25::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant3(__nt), __end));
        (2, 2)
    }
    fn __reduce7<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Dictionary = "{", Statement+, "}" => ActionFn(26);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant0(__symbols);
        let __sym1 = __pop_Variant5(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action26::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant3(__nt), __end));
        (3, 2)
    }
    fn __reduce8<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = atom, "=", Data => ActionFn(6);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant2(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action6::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (3, 3)
    }
    fn __reduce9<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = Block => ActionFn(7);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action7::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (1, 3)
    }
    fn __reduce10<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = atom, ":", Data, "," => ActionFn(8);
        assert!(__symbols.len() >= 4);
        let __sym3 = __pop_Variant0(__symbols);
        let __sym2 = __pop_Variant2(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym3.2;
        let __nt = super::__action8::<>(__sym0, __sym1, __sym2, __sym3);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (4, 3)
    }
    fn __reduce11<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = Data, ":", Data, "," => ActionFn(9);
        assert!(__symbols.len() >= 4);
        let __sym3 = __pop_Variant0(__symbols);
        let __sym2 = __pop_Variant2(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant2(__symbols);
        let __start = __sym0.0;
        let __end = __sym3.2;
        let __nt = super::__action9::<>(__sym0, __sym1, __sym2, __sym3);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (4, 3)
    }
    fn __reduce12<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = atom, ":", Dictionary => ActionFn(10);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant3(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action10::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (3, 3)
    }
    fn __reduce13<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = Data, ":", Dictionary => ActionFn(11);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant3(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant2(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action11::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (3, 3)
    }
    fn __reduce14<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = atom, "," => ActionFn(12);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action12::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (2, 3)
    }
    fn __reduce15<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement* =  => ActionFn(21);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action21::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant5(__nt), __end));
        (0, 4)
    }
    fn __reduce16<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement* = Statement+ => ActionFn(22);
        let __sym0 = __pop_Variant5(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action22::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant5(__nt), __end));
        (1, 4)
    }
    fn __reduce17<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement+ = Statement => ActionFn(23);
        let __sym0 = __pop_Variant4(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action23::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant5(__nt), __end));
        (1, 5)
    }
    fn __reduce18<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement+ = Statement+, Statement => ActionFn(24);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant4(__symbols);
        let __sym0 = __pop_Variant5(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action24::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant5(__nt), __end));
        (2, 5)
    }
    fn __reduce19<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statements =  => ActionFn(27);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action27::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant6(__nt), __end));
        (0, 6)
    }
    fn __reduce20<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statements = Statement+ => ActionFn(28);
        let __sym0 = __pop_Variant5(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action28::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant6(__nt), __end));
        (1, 6)
    }
    fn __reduce21<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // __Block = Block => ActionFn(2);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action2::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (1, 7)
    }
    fn __reduce22<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // __Data = Data => ActionFn(4);
        let __sym0 = __pop_Variant2(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action4::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (1, 8)
    }
    fn __reduce24<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // __Statement = Statement => ActionFn(1);
        let __sym0 = __pop_Variant4(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action1::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (1, 10)
    }
    fn __reduce25<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // __Statements = Statements => ActionFn(0);
        let __sym0 = __pop_Variant6(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action0::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant6(__nt), __end));
        (1, 11)
    }
    fn __reduce26<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // sqs? = sqs => ActionFn(19);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action19::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant7(__nt), __end));
        (1, 12)
    }
    fn __reduce27<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // sqs? =  => ActionFn(20);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action20::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant7(__nt), __end));
        (0, 12)
    }
}
#[allow(unused_imports)]
pub use self::__parse__Dictionary::DictionaryParser;

#[rustfmt::skip]
#[allow(explicit_outlives_requirements, non_snake_case, non_camel_case_types, unused_mut, unused_variables, unused_imports, unused_parens, clippy::needless_lifetimes, clippy::type_complexity, clippy::needless_return, clippy::too_many_arguments, clippy::never_loop, clippy::match_single_binding, clippy::needless_raw_string_hashes)]
mod __parse__Statement {

    use crate::lexer::{self, Token};
    use crate::{Block, Data, Dictionary, Statement};
    #[allow(unused_extern_crates)]
    extern crate lalrpop_util as __lalrpop_util;
    #[allow(unused_imports)]
    use self::__lalrpop_util::state_machine as __state_machine;
    #[allow(unused_extern_crates)]
    extern crate alloc;
    use super::__ToTriple;
    #[allow(dead_code)]
    pub(crate) enum __Symbol<>
     {
        Variant0(lexer::Token),
        Variant1(Block),
        Variant2(Data),
        Variant3(Dictionary),
        Variant4(Statement),
        Variant5(alloc::vec::Vec<Statement>),
        Variant6(Vec<Statement>),
        Variant7(Option<lexer::Token>),
    }
    const __ACTION: &[i8] = &[
        // State 0
        0, 0, 0, 0, 12, 0, 0, 0, 0, 2, 14, 15, 13, 0,
        // State 1
        18, 0, 5, 4, 0, 6, 0, 0, 0, 0, 7, 0, 0, 0,
        // State 2
        0, 0, 0, 0, 12, 6, 0, 0, 0, 0, 14, 15, 13, 0,
        // State 3
        0, 0, 0, 0, 12, 6, 0, 0, 0, 0, 14, 15, 13, 0,
        // State 4
        0, 0, 0, 0, 12, 0, 0, 0, 0, 0, 14, 15, 13, 0,
        // State 5
        0, 0, 0, 0, 12, 0, 25, 0, 0, 2, 14, 15, 13, 0,
        // State 6
        0, 0, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 7
        0, 0, 0, 0, 12, 0, 30, 0, 0, 2, 14, 15, 13, 0,
        // State 8
        0, 0, 0, 0, -10, 0, -10, 0, 0, -10, -10, -10, -10, 0,
        // State 9
        0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 10
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 11
        0, 0, 0, 0, 0, 0, 0, 0, 0, 16, 0, 0, 0, 0,
        // State 12
        -5, 0, 0, -5, -5, 0, -5, 0, 0, -5, -5, -5, -5, 0,
        // State 13
        -3, 0, 0, -3, -3, 0, -3, 0, 0, -3, -3, -3, -3, 0,
        // State 14
        -4, 0, 0, -4, -4, 0, -4, 0, 0, -4, -4, -4, -4, 0,
        // State 15
        -6, 0, 0, -6, -6, 0, -6, 0, 0, -6, -6, -6, -6, 0,
        // State 16
        0, 0, 0, 0, -2, 0, -2, 0, 0, -2, -2, -2, -2, 0,
        // State 17
        0, 0, 0, 0, -15, 0, -15, 0, 0, -15, -15, -15, -15, 0,
        // State 18
        27, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 19
        0, 0, 0, 0, -14, 0, -14, 0, 0, -14, -14, -14, -14, 0,
        // State 20
        28, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 21
        0, 0, 0, 0, -13, 0, -13, 0, 0, -13, -13, -13, -13, 0,
        // State 22
        0, 0, 0, 0, -9, 0, -9, 0, 0, -9, -9, -9, -9, 0,
        // State 23
        0, 0, 0, 0, -18, 0, -18, 0, 0, -18, -18, -18, -18, 0,
        // State 24
        0, 0, 0, 0, -7, 0, -7, 0, 0, -7, -7, -7, -7, 0,
        // State 25
        0, 0, 0, 0, -1, 0, -1, 0, 0, -1, -1, -1, -1, 0,
        // State 26
        0, 0, 0, 0, -12, 0, -12, 0, 0, -12, -12, -12, -12, 0,
        // State 27
        0, 0, 0, 0, -11, 0, -11, 0, 0, -11, -11, -11, -11, 0,
        // State 28
        0, 0, 0, 0, -19, 0, -19, 0, 0, -19, -19, -19, -19, 0,
        // State 29
        0, 0, 0, 0, -8, 0, -8, 0, 0, -8, -8, -8, -8, 0,
    ];
    fn __action(state: i8, integer: usize) -> i8 {
        __ACTION[(state as usize) * 14 + integer]
    }
    const __EOF_ACTION: &[i8] = &[
        // State 0
        0,
        // State 1
        0,
        // State 2
        0,
        // State 3
        0,
        // State 4
        0,
        // State 5
        0,
        // State 6
        0,
        // State 7
        0,
        // State 8
        -10,
        // State 9
        0,
        // State 10
        -25,
        // State 11
        0,
        // State 12
        -5,
        // State 13
        -3,
        // State 14
        -4,
        // State 15
        -6,
        // State 16
        -2,
        // State 17
        -15,
        // State 18
        0,
        // State 19
        -14,
        // State 20
        0,
        // State 21
        -13,
        // State 22
        -9,
        // State 23
        0,
        // State 24
        -7,
        // State 25
        -1,
        // State 26
        -12,
        // State 27
        -11,
        // State 28
        0,
        // State 29
        -8,
    ];
    fn __goto(state: i8, nt: usize) -> i8 {
        match nt {
            0 => 8,
            1 => match state {
                2 => 18,
                3 => 20,
                4 => 22,
                _ => 9,
            },
            2 => match state {
                2 => 19,
                3 => 21,
                6 => 25,
                _ => 16,
            },
            3 => match state {
                5 => 23,
                7 => 28,
                _ => 10,
            },
            5 => 7,
            _ => 0,
        }
    }
    const __TERMINAL: &[&str] = &[
        r###"",""###,
        r###""%""###,
        r###""=""###,
        r###"":""###,
        r###""$""###,
        r###""{""###,
        r###""}""###,
        r###""[""###,
        r###""]""###,
        r###"atom"###,
        r###"sqs"###,
        r###"tqs"###,
        r###"f64"###,
        r###"comment"###,
    ];
    fn __expected_tokens(__state: i8) -> alloc::vec::Vec<alloc::string::String> {
        __TERMINAL.iter().enumerate().filter_map(|(index, terminal)| {
            let next_state = __action(__state, index);
            if next_state == 0 {
                None
            } else {
                Some(alloc::string::ToString::to_string(terminal))
            }
        }).collect()
    }
    fn __expected_tokens_from_states<
    >(
        __states: &[i8],
        _: core::marker::PhantomData<()>,
    ) -> alloc::vec::Vec<alloc::string::String>
    {
        __TERMINAL.iter().enumerate().filter_map(|(index, terminal)| {
            if __accepts(None, __states, Some(index), core::marker::PhantomData::<()>) {
                Some(alloc::string::ToString::to_string(terminal))
            } else {
                None
            }
        }).collect()
    }
    struct __StateMachine<>
    where 
    {
        __phantom: core::marker::PhantomData<()>,
    }
    impl<> __state_machine::ParserDefinition for __StateMachine<>
    where 
    {
        type Location = lexer::Location;
        type Error = lexer::LexicalError;
        type Token = lexer::Token;
        type TokenIndex = usize;
        type Symbol = __Symbol<>;
        type Success = Statement;
        type StateIndex = i8;
        type Action = i8;
        type ReduceIndex = i8;
        type NonterminalIndex = usize;

        #[inline]
        fn start_location(&self) -> Self::Location {
              Default::default()
        }

        #[inline]
        fn start_state(&self) -> Self::StateIndex {
              0
        }

        #[inline]
        fn token_to_index(&self, token: &Self::Token) -> Option<usize> {
            __token_to_integer(token, core::marker::PhantomData::<()>)
        }

        #[inline]
        fn action(&self, state: i8, integer: usize) -> i8 {
            __action(state, integer)
        }

        #[inline]
        fn error_action(&self, state: i8) -> i8 {
            __action(state, 14 - 1)
        }

        #[inline]
        fn eof_action(&self, state: i8) -> i8 {
            __EOF_ACTION[state as usize]
        }

        #[inline]
        fn goto(&self, state: i8, nt: usize) -> i8 {
            __goto(state, nt)
        }

        fn token_to_symbol(&self, token_index: usize, token: Self::Token) -> Self::Symbol {
            __token_to_symbol(token_index, token, core::marker::PhantomData::<()>)
        }

        fn expected_tokens(&self, state: i8) -> alloc::vec::Vec<alloc::string::String> {
            __expected_tokens(state)
        }

        fn expected_tokens_from_states(&self, states: &[i8]) -> alloc::vec::Vec<alloc::string::String> {
            __expected_tokens_from_states(states, core::marker::PhantomData::<()>)
        }

        #[inline]
        fn uses_error_recovery(&self) -> bool {
            false
        }

        #[inline]
        fn error_recovery_symbol(
            &self,
            recovery: __state_machine::ErrorRecovery<Self>,
        ) -> Self::Symbol {
            panic!("error recovery not enabled for this grammar")
        }

        fn reduce(
            &mut self,
            action: i8,
            start_location: Option<&Self::Location>,
            states: &mut alloc::vec::Vec<i8>,
            symbols: &mut alloc::vec::Vec<__state_machine::SymbolTriple<Self>>,
        ) -> Option<__state_machine::ParseResult<Self>> {
            __reduce(
                action,
                start_location,
                states,
                symbols,
                core::marker::PhantomData::<()>,
            )
        }

        fn simulate_reduce(&self, action: i8) -> __state_machine::SimulatedReduce<Self> {
            __simulate_reduce(action, core::marker::PhantomData::<()>)
        }
    }
    fn __token_to_integer<
    >(
        __token: &lexer::Token,
        _: core::marker::PhantomData<()>,
    ) -> Option<usize>
    {
        #[warn(unused_variables)]
        match __token {
            Token::Comma if true => Some(0),
            Token::Percent if true => Some(1),
            Token::Equals if true => Some(2),
            Token::Colon if true => Some(3),
            Token::DollarSign if true => Some(4),
            Token::LeftBrace if true => Some(5),
            Token::RightBrace if true => Some(6),
            Token::LeftBracket if true => Some(7),
            Token::RightBracket if true => Some(8),
            Token::Atom(_) if true => Some(9),
            Token::SingleQuotedString(_) if true => Some(10),
            Token::TripleQuotedString(_) if true => Some(11),
            Token::F64(_) if true => Some(12),
            Token::Comment(_) if true => Some(13),
            _ => None,
        }
    }
    fn __token_to_symbol<
    >(
        __token_index: usize,
        __token: lexer::Token,
        _: core::marker::PhantomData<()>,
    ) -> __Symbol<>
    {
        #[allow(clippy::manual_range_patterns)]match __token_index {
            0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 => __Symbol::Variant0(__token),
            _ => unreachable!(),
        }
    }
    fn __simulate_reduce<
    >(
        __reduce_index: i8,
        _: core::marker::PhantomData<()>,
    ) -> __state_machine::SimulatedReduce<__StateMachine<>>
    {
        match __reduce_index {
            0 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 0,
                }
            }
            1 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 0,
                }
            }
            2 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 1,
                }
            }
            3 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 1,
                }
            }
            4 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 1,
                }
            }
            5 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 1,
                }
            }
            6 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 2,
                }
            }
            7 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 2,
                }
            }
            8 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 3,
                }
            }
            9 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 3,
                }
            }
            10 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 4,
                    nonterminal_produced: 3,
                }
            }
            11 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 4,
                    nonterminal_produced: 3,
                }
            }
            12 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 3,
                }
            }
            13 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 3,
                }
            }
            14 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 3,
                }
            }
            15 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 4,
                }
            }
            16 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 4,
                }
            }
            17 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 5,
                }
            }
            18 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 5,
                }
            }
            19 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 6,
                }
            }
            20 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 6,
                }
            }
            21 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 7,
                }
            }
            22 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 8,
                }
            }
            23 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 9,
                }
            }
            24 => __state_machine::SimulatedReduce::Accept,
            25 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 11,
                }
            }
            26 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 12,
                }
            }
            27 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 12,
                }
            }
            _ => panic!("invalid reduction index {}", __reduce_index)
        }
    }
    pub struct StatementParser {
        _priv: (),
    }

    impl Default for StatementParser { fn default() -> Self { Self::new() } }
    impl StatementParser {
        pub fn new() -> StatementParser {
            StatementParser {
                _priv: (),
            }
        }

        #[allow(dead_code)]
        pub fn parse<
            __TOKEN: __ToTriple<>,
            __TOKENS: IntoIterator<Item=__TOKEN>,
        >(
            &self,
            __tokens0: __TOKENS,
        ) -> Result<Statement, __lalrpop_util::ParseError<lexer::Location, lexer::Token, lexer::LexicalError>>
        {
            let __tokens = __tokens0.into_iter();
            let mut __tokens = __tokens.map(|t| __ToTriple::to_triple(t));
            __state_machine::Parser::drive(
                __StateMachine {
                    __phantom: core::marker::PhantomData::<()>,
                },
                __tokens,
            )
        }
    }
    fn __accepts<
    >(
        __error_state: Option<i8>,
        __states: &[i8],
        __opt_integer: Option<usize>,
        _: core::marker::PhantomData<()>,
    ) -> bool
    {
        let mut __states = __states.to_vec();
        __states.extend(__error_state);
        loop {
            let mut __states_len = __states.len();
            let __top = __states[__states_len - 1];
            let __action = match __opt_integer {
                None => __EOF_ACTION[__top as usize],
                Some(__integer) => __action(__top, __integer),
            };
            if __action == 0 { return false; }
            if __action > 0 { return true; }
            let (__to_pop, __nt) = match __simulate_reduce(-(__action + 1), core::marker::PhantomData::<()>) {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop, nonterminal_produced
                } => (states_to_pop, nonterminal_produced),
                __state_machine::SimulatedReduce::Accept => return true,
            };
            __states_len -= __to_pop;
            __states.truncate(__states_len);
            let __top = __states[__states_len - 1];
            let __next_state = __goto(__top, __nt);
            __states.push(__next_state);
        }
    }
    fn __reduce<
    >(
        __action: i8,
        __lookahead_start: Option<&lexer::Location>,
        __states: &mut alloc::vec::Vec<i8>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> Option<Result<Statement,__lalrpop_util::ParseError<lexer::Location, lexer::Token, lexer::LexicalError>>>
    {
        let (__pop_states, __nonterminal) = match __action {
            0 => {
                __reduce0(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            1 => {
                __reduce1(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            2 => {
                __reduce2(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            3 => {
                __reduce3(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            4 => {
                __reduce4(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            5 => {
                __reduce5(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            6 => {
                __reduce6(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            7 => {
                __reduce7(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            8 => {
                __reduce8(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            9 => {
                __reduce9(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            10 => {
                __reduce10(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            11 => {
                __reduce11(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            12 => {
                __reduce12(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            13 => {
                __reduce13(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            14 => {
                __reduce14(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            15 => {
                __reduce15(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            16 => {
                __reduce16(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            17 => {
                __reduce17(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            18 => {
                __reduce18(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            19 => {
                __reduce19(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            20 => {
                __reduce20(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            21 => {
                __reduce21(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            22 => {
                __reduce22(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            23 => {
                __reduce23(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            24 => {
                // __Statement = Statement => ActionFn(1);
                let __sym0 = __pop_Variant4(__symbols);
                let __start = __sym0.0;
                let __end = __sym0.2;
                let __nt = super::__action1::<>(__sym0);
                return Some(Ok(__nt));
            }
            25 => {
                __reduce25(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            26 => {
                __reduce26(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            27 => {
                __reduce27(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            _ => panic!("invalid action code {}", __action)
        };
        let __states_len = __states.len();
        __states.truncate(__states_len - __pop_states);
        let __state = *__states.last().unwrap();
        let __next_state = __goto(__state, __nonterminal);
        __states.push(__next_state);
        None
    }
    #[inline(never)]
    fn __symbol_type_mismatch() -> ! {
        panic!("symbol type mismatch")
    }
    fn __pop_Variant1<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Block, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant1(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant2<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Data, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant2(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant3<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Dictionary, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant3(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant7<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Option<lexer::Token>, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant7(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant4<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Statement, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant4(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant6<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Vec<Statement>, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant6(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant5<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, alloc::vec::Vec<Statement>, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant5(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant0<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, lexer::Token, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant0(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __reduce0<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Block = atom, sqs, Dictionary => ActionFn(29);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant3(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action29::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (3, 0)
    }
    fn __reduce1<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Block = atom, Dictionary => ActionFn(30);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant3(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action30::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (2, 0)
    }
    fn __reduce2<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Data = sqs => ActionFn(15);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action15::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (1, 1)
    }
    fn __reduce3<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Data = tqs => ActionFn(16);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action16::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (1, 1)
    }
    fn __reduce4<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Data = f64 => ActionFn(17);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action17::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (1, 1)
    }
    fn __reduce5<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Data = "$", atom => ActionFn(18);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action18::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (2, 1)
    }
    fn __reduce6<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Dictionary = "{", "}" => ActionFn(25);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action25::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant3(__nt), __end));
        (2, 2)
    }
    fn __reduce7<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Dictionary = "{", Statement+, "}" => ActionFn(26);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant0(__symbols);
        let __sym1 = __pop_Variant5(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action26::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant3(__nt), __end));
        (3, 2)
    }
    fn __reduce8<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = atom, "=", Data => ActionFn(6);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant2(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action6::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (3, 3)
    }
    fn __reduce9<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = Block => ActionFn(7);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action7::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (1, 3)
    }
    fn __reduce10<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = atom, ":", Data, "," => ActionFn(8);
        assert!(__symbols.len() >= 4);
        let __sym3 = __pop_Variant0(__symbols);
        let __sym2 = __pop_Variant2(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym3.2;
        let __nt = super::__action8::<>(__sym0, __sym1, __sym2, __sym3);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (4, 3)
    }
    fn __reduce11<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = Data, ":", Data, "," => ActionFn(9);
        assert!(__symbols.len() >= 4);
        let __sym3 = __pop_Variant0(__symbols);
        let __sym2 = __pop_Variant2(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant2(__symbols);
        let __start = __sym0.0;
        let __end = __sym3.2;
        let __nt = super::__action9::<>(__sym0, __sym1, __sym2, __sym3);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (4, 3)
    }
    fn __reduce12<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = atom, ":", Dictionary => ActionFn(10);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant3(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action10::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (3, 3)
    }
    fn __reduce13<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = Data, ":", Dictionary => ActionFn(11);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant3(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant2(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action11::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (3, 3)
    }
    fn __reduce14<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = atom, "," => ActionFn(12);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action12::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (2, 3)
    }
    fn __reduce15<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement* =  => ActionFn(21);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action21::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant5(__nt), __end));
        (0, 4)
    }
    fn __reduce16<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement* = Statement+ => ActionFn(22);
        let __sym0 = __pop_Variant5(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action22::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant5(__nt), __end));
        (1, 4)
    }
    fn __reduce17<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement+ = Statement => ActionFn(23);
        let __sym0 = __pop_Variant4(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action23::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant5(__nt), __end));
        (1, 5)
    }
    fn __reduce18<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement+ = Statement+, Statement => ActionFn(24);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant4(__symbols);
        let __sym0 = __pop_Variant5(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action24::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant5(__nt), __end));
        (2, 5)
    }
    fn __reduce19<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statements =  => ActionFn(27);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action27::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant6(__nt), __end));
        (0, 6)
    }
    fn __reduce20<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statements = Statement+ => ActionFn(28);
        let __sym0 = __pop_Variant5(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action28::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant6(__nt), __end));
        (1, 6)
    }
    fn __reduce21<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // __Block = Block => ActionFn(2);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action2::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (1, 7)
    }
    fn __reduce22<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // __Data = Data => ActionFn(4);
        let __sym0 = __pop_Variant2(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action4::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (1, 8)
    }
    fn __reduce23<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // __Dictionary = Dictionary => ActionFn(3);
        let __sym0 = __pop_Variant3(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action3::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant3(__nt), __end));
        (1, 9)
    }
    fn __reduce25<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // __Statements = Statements => ActionFn(0);
        let __sym0 = __pop_Variant6(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action0::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant6(__nt), __end));
        (1, 11)
    }
    fn __reduce26<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // sqs? = sqs => ActionFn(19);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action19::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant7(__nt), __end));
        (1, 12)
    }
    fn __reduce27<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // sqs? =  => ActionFn(20);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action20::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant7(__nt), __end));
        (0, 12)
    }
}
#[allow(unused_imports)]
pub use self::__parse__Statement::StatementParser;

#[rustfmt::skip]
#[allow(explicit_outlives_requirements, non_snake_case, non_camel_case_types, unused_mut, unused_variables, unused_imports, unused_parens, clippy::needless_lifetimes, clippy::type_complexity, clippy::needless_return, clippy::too_many_arguments, clippy::never_loop, clippy::match_single_binding, clippy::needless_raw_string_hashes)]
mod __parse__Statements {

    use crate::lexer::{self, Token};
    use crate::{Block, Data, Dictionary, Statement};
    #[allow(unused_extern_crates)]
    extern crate lalrpop_util as __lalrpop_util;
    #[allow(unused_imports)]
    use self::__lalrpop_util::state_machine as __state_machine;
    #[allow(unused_extern_crates)]
    extern crate alloc;
    use super::__ToTriple;
    #[allow(dead_code)]
    pub(crate) enum __Symbol<>
     {
        Variant0(lexer::Token),
        Variant1(Block),
        Variant2(Data),
        Variant3(Dictionary),
        Variant4(Statement),
        Variant5(alloc::vec::Vec<Statement>),
        Variant6(Vec<Statement>),
        Variant7(Option<lexer::Token>),
    }
    const __ACTION: &[i8] = &[
        // State 0
        0, 0, 0, 0, 14, 0, 0, 0, 0, 3, 16, 17, 15, 0,
        // State 1
        0, 0, 0, 0, 14, 0, 0, 0, 0, 3, 16, 17, 15, 0,
        // State 2
        21, 0, 6, 5, 0, 7, 0, 0, 0, 0, 8, 0, 0, 0,
        // State 3
        0, 0, 0, 0, 14, 7, 0, 0, 0, 0, 16, 17, 15, 0,
        // State 4
        0, 0, 0, 0, 14, 7, 0, 0, 0, 0, 16, 17, 15, 0,
        // State 5
        0, 0, 0, 0, 14, 0, 0, 0, 0, 0, 16, 17, 15, 0,
        // State 6
        0, 0, 0, 0, 14, 0, 27, 0, 0, 3, 16, 17, 15, 0,
        // State 7
        0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 8
        0, 0, 0, 0, 14, 0, 31, 0, 0, 3, 16, 17, 15, 0,
        // State 9
        0, 0, 0, 0, -10, 0, -10, 0, 0, -10, -10, -10, -10, 0,
        // State 10
        0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 11
        0, 0, 0, 0, -18, 0, -18, 0, 0, -18, -18, -18, -18, 0,
        // State 12
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 13
        0, 0, 0, 0, 0, 0, 0, 0, 0, 19, 0, 0, 0, 0,
        // State 14
        -5, 0, 0, -5, -5, 0, -5, 0, 0, -5, -5, -5, -5, 0,
        // State 15
        -3, 0, 0, -3, -3, 0, -3, 0, 0, -3, -3, -3, -3, 0,
        // State 16
        -4, 0, 0, -4, -4, 0, -4, 0, 0, -4, -4, -4, -4, 0,
        // State 17
        0, 0, 0, 0, -19, 0, -19, 0, 0, -19, -19, -19, -19, 0,
        // State 18
        -6, 0, 0, -6, -6, 0, -6, 0, 0, -6, -6, -6, -6, 0,
        // State 19
        0, 0, 0, 0, -2, 0, -2, 0, 0, -2, -2, -2, -2, 0,
        // State 20
        0, 0, 0, 0, -15, 0, -15, 0, 0, -15, -15, -15, -15, 0,
        // State 21
        29, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 22
        0, 0, 0, 0, -14, 0, -14, 0, 0, -14, -14, -14, -14, 0,
        // State 23
        30, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 24
        0, 0, 0, 0, -13, 0, -13, 0, 0, -13, -13, -13, -13, 0,
        // State 25
        0, 0, 0, 0, -9, 0, -9, 0, 0, -9, -9, -9, -9, 0,
        // State 26
        0, 0, 0, 0, -7, 0, -7, 0, 0, -7, -7, -7, -7, 0,
        // State 27
        0, 0, 0, 0, -1, 0, -1, 0, 0, -1, -1, -1, -1, 0,
        // State 28
        0, 0, 0, 0, -12, 0, -12, 0, 0, -12, -12, -12, -12, 0,
        // State 29
        0, 0, 0, 0, -11, 0, -11, 0, 0, -11, -11, -11, -11, 0,
        // State 30
        0, 0, 0, 0, -8, 0, -8, 0, 0, -8, -8, -8, -8, 0,
    ];
    fn __action(state: i8, integer: usize) -> i8 {
        __ACTION[(state as usize) * 14 + integer]
    }
    const __EOF_ACTION: &[i8] = &[
        // State 0
        -20,
        // State 1
        -21,
        // State 2
        0,
        // State 3
        0,
        // State 4
        0,
        // State 5
        0,
        // State 6
        0,
        // State 7
        0,
        // State 8
        0,
        // State 9
        -10,
        // State 10
        0,
        // State 11
        -18,
        // State 12
        -26,
        // State 13
        0,
        // State 14
        -5,
        // State 15
        -3,
        // State 16
        -4,
        // State 17
        -19,
        // State 18
        -6,
        // State 19
        -2,
        // State 20
        -15,
        // State 21
        0,
        // State 22
        -14,
        // State 23
        0,
        // State 24
        -13,
        // State 25
        -9,
        // State 26
        -7,
        // State 27
        -1,
        // State 28
        -12,
        // State 29
        -11,
        // State 30
        -8,
    ];
    fn __goto(state: i8, nt: usize) -> i8 {
        match nt {
            0 => 9,
            1 => match state {
                3 => 21,
                4 => 23,
                5 => 25,
                _ => 10,
            },
            2 => match state {
                3 => 22,
                4 => 24,
                7 => 27,
                _ => 19,
            },
            3 => match state {
                1 | 8 => 17,
                _ => 11,
            },
            5 => match state {
                6 => 8,
                _ => 1,
            },
            6 => 12,
            _ => 0,
        }
    }
    const __TERMINAL: &[&str] = &[
        r###"",""###,
        r###""%""###,
        r###""=""###,
        r###"":""###,
        r###""$""###,
        r###""{""###,
        r###""}""###,
        r###""[""###,
        r###""]""###,
        r###"atom"###,
        r###"sqs"###,
        r###"tqs"###,
        r###"f64"###,
        r###"comment"###,
    ];
    fn __expected_tokens(__state: i8) -> alloc::vec::Vec<alloc::string::String> {
        __TERMINAL.iter().enumerate().filter_map(|(index, terminal)| {
            let next_state = __action(__state, index);
            if next_state == 0 {
                None
            } else {
                Some(alloc::string::ToString::to_string(terminal))
            }
        }).collect()
    }
    fn __expected_tokens_from_states<
    >(
        __states: &[i8],
        _: core::marker::PhantomData<()>,
    ) -> alloc::vec::Vec<alloc::string::String>
    {
        __TERMINAL.iter().enumerate().filter_map(|(index, terminal)| {
            if __accepts(None, __states, Some(index), core::marker::PhantomData::<()>) {
                Some(alloc::string::ToString::to_string(terminal))
            } else {
                None
            }
        }).collect()
    }
    struct __StateMachine<>
    where 
    {
        __phantom: core::marker::PhantomData<()>,
    }
    impl<> __state_machine::ParserDefinition for __StateMachine<>
    where 
    {
        type Location = lexer::Location;
        type Error = lexer::LexicalError;
        type Token = lexer::Token;
        type TokenIndex = usize;
        type Symbol = __Symbol<>;
        type Success = Vec<Statement>;
        type StateIndex = i8;
        type Action = i8;
        type ReduceIndex = i8;
        type NonterminalIndex = usize;

        #[inline]
        fn start_location(&self) -> Self::Location {
              Default::default()
        }

        #[inline]
        fn start_state(&self) -> Self::StateIndex {
              0
        }

        #[inline]
        fn token_to_index(&self, token: &Self::Token) -> Option<usize> {
            __token_to_integer(token, core::marker::PhantomData::<()>)
        }

        #[inline]
        fn action(&self, state: i8, integer: usize) -> i8 {
            __action(state, integer)
        }

        #[inline]
        fn error_action(&self, state: i8) -> i8 {
            __action(state, 14 - 1)
        }

        #[inline]
        fn eof_action(&self, state: i8) -> i8 {
            __EOF_ACTION[state as usize]
        }

        #[inline]
        fn goto(&self, state: i8, nt: usize) -> i8 {
            __goto(state, nt)
        }

        fn token_to_symbol(&self, token_index: usize, token: Self::Token) -> Self::Symbol {
            __token_to_symbol(token_index, token, core::marker::PhantomData::<()>)
        }

        fn expected_tokens(&self, state: i8) -> alloc::vec::Vec<alloc::string::String> {
            __expected_tokens(state)
        }

        fn expected_tokens_from_states(&self, states: &[i8]) -> alloc::vec::Vec<alloc::string::String> {
            __expected_tokens_from_states(states, core::marker::PhantomData::<()>)
        }

        #[inline]
        fn uses_error_recovery(&self) -> bool {
            false
        }

        #[inline]
        fn error_recovery_symbol(
            &self,
            recovery: __state_machine::ErrorRecovery<Self>,
        ) -> Self::Symbol {
            panic!("error recovery not enabled for this grammar")
        }

        fn reduce(
            &mut self,
            action: i8,
            start_location: Option<&Self::Location>,
            states: &mut alloc::vec::Vec<i8>,
            symbols: &mut alloc::vec::Vec<__state_machine::SymbolTriple<Self>>,
        ) -> Option<__state_machine::ParseResult<Self>> {
            __reduce(
                action,
                start_location,
                states,
                symbols,
                core::marker::PhantomData::<()>,
            )
        }

        fn simulate_reduce(&self, action: i8) -> __state_machine::SimulatedReduce<Self> {
            __simulate_reduce(action, core::marker::PhantomData::<()>)
        }
    }
    fn __token_to_integer<
    >(
        __token: &lexer::Token,
        _: core::marker::PhantomData<()>,
    ) -> Option<usize>
    {
        #[warn(unused_variables)]
        match __token {
            Token::Comma if true => Some(0),
            Token::Percent if true => Some(1),
            Token::Equals if true => Some(2),
            Token::Colon if true => Some(3),
            Token::DollarSign if true => Some(4),
            Token::LeftBrace if true => Some(5),
            Token::RightBrace if true => Some(6),
            Token::LeftBracket if true => Some(7),
            Token::RightBracket if true => Some(8),
            Token::Atom(_) if true => Some(9),
            Token::SingleQuotedString(_) if true => Some(10),
            Token::TripleQuotedString(_) if true => Some(11),
            Token::F64(_) if true => Some(12),
            Token::Comment(_) if true => Some(13),
            _ => None,
        }
    }
    fn __token_to_symbol<
    >(
        __token_index: usize,
        __token: lexer::Token,
        _: core::marker::PhantomData<()>,
    ) -> __Symbol<>
    {
        #[allow(clippy::manual_range_patterns)]match __token_index {
            0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 => __Symbol::Variant0(__token),
            _ => unreachable!(),
        }
    }
    fn __simulate_reduce<
    >(
        __reduce_index: i8,
        _: core::marker::PhantomData<()>,
    ) -> __state_machine::SimulatedReduce<__StateMachine<>>
    {
        match __reduce_index {
            0 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 0,
                }
            }
            1 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 0,
                }
            }
            2 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 1,
                }
            }
            3 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 1,
                }
            }
            4 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 1,
                }
            }
            5 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 1,
                }
            }
            6 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 2,
                }
            }
            7 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 2,
                }
            }
            8 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 3,
                }
            }
            9 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 3,
                }
            }
            10 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 4,
                    nonterminal_produced: 3,
                }
            }
            11 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 4,
                    nonterminal_produced: 3,
                }
            }
            12 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 3,
                }
            }
            13 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 3,
                }
            }
            14 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 3,
                }
            }
            15 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 4,
                }
            }
            16 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 4,
                }
            }
            17 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 5,
                }
            }
            18 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 5,
                }
            }
            19 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 6,
                }
            }
            20 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 6,
                }
            }
            21 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 7,
                }
            }
            22 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 8,
                }
            }
            23 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 9,
                }
            }
            24 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 10,
                }
            }
            25 => __state_machine::SimulatedReduce::Accept,
            26 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 12,
                }
            }
            27 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 12,
                }
            }
            _ => panic!("invalid reduction index {}", __reduce_index)
        }
    }
    pub struct StatementsParser {
        _priv: (),
    }

    impl Default for StatementsParser { fn default() -> Self { Self::new() } }
    impl StatementsParser {
        pub fn new() -> StatementsParser {
            StatementsParser {
                _priv: (),
            }
        }

        #[allow(dead_code)]
        pub fn parse<
            __TOKEN: __ToTriple<>,
            __TOKENS: IntoIterator<Item=__TOKEN>,
        >(
            &self,
            __tokens0: __TOKENS,
        ) -> Result<Vec<Statement>, __lalrpop_util::ParseError<lexer::Location, lexer::Token, lexer::LexicalError>>
        {
            let __tokens = __tokens0.into_iter();
            let mut __tokens = __tokens.map(|t| __ToTriple::to_triple(t));
            __state_machine::Parser::drive(
                __StateMachine {
                    __phantom: core::marker::PhantomData::<()>,
                },
                __tokens,
            )
        }
    }
    fn __accepts<
    >(
        __error_state: Option<i8>,
        __states: &[i8],
        __opt_integer: Option<usize>,
        _: core::marker::PhantomData<()>,
    ) -> bool
    {
        let mut __states = __states.to_vec();
        __states.extend(__error_state);
        loop {
            let mut __states_len = __states.len();
            let __top = __states[__states_len - 1];
            let __action = match __opt_integer {
                None => __EOF_ACTION[__top as usize],
                Some(__integer) => __action(__top, __integer),
            };
            if __action == 0 { return false; }
            if __action > 0 { return true; }
            let (__to_pop, __nt) = match __simulate_reduce(-(__action + 1), core::marker::PhantomData::<()>) {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop, nonterminal_produced
                } => (states_to_pop, nonterminal_produced),
                __state_machine::SimulatedReduce::Accept => return true,
            };
            __states_len -= __to_pop;
            __states.truncate(__states_len);
            let __top = __states[__states_len - 1];
            let __next_state = __goto(__top, __nt);
            __states.push(__next_state);
        }
    }
    fn __reduce<
    >(
        __action: i8,
        __lookahead_start: Option<&lexer::Location>,
        __states: &mut alloc::vec::Vec<i8>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> Option<Result<Vec<Statement>,__lalrpop_util::ParseError<lexer::Location, lexer::Token, lexer::LexicalError>>>
    {
        let (__pop_states, __nonterminal) = match __action {
            0 => {
                __reduce0(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            1 => {
                __reduce1(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            2 => {
                __reduce2(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            3 => {
                __reduce3(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            4 => {
                __reduce4(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            5 => {
                __reduce5(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            6 => {
                __reduce6(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            7 => {
                __reduce7(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            8 => {
                __reduce8(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            9 => {
                __reduce9(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            10 => {
                __reduce10(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            11 => {
                __reduce11(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            12 => {
                __reduce12(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            13 => {
                __reduce13(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            14 => {
                __reduce14(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            15 => {
                __reduce15(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            16 => {
                __reduce16(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            17 => {
                __reduce17(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            18 => {
                __reduce18(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            19 => {
                __reduce19(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            20 => {
                __reduce20(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            21 => {
                __reduce21(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            22 => {
                __reduce22(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            23 => {
                __reduce23(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            24 => {
                __reduce24(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            25 => {
                // __Statements = Statements => ActionFn(0);
                let __sym0 = __pop_Variant6(__symbols);
                let __start = __sym0.0;
                let __end = __sym0.2;
                let __nt = super::__action0::<>(__sym0);
                return Some(Ok(__nt));
            }
            26 => {
                __reduce26(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            27 => {
                __reduce27(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            _ => panic!("invalid action code {}", __action)
        };
        let __states_len = __states.len();
        __states.truncate(__states_len - __pop_states);
        let __state = *__states.last().unwrap();
        let __next_state = __goto(__state, __nonterminal);
        __states.push(__next_state);
        None
    }
    #[inline(never)]
    fn __symbol_type_mismatch() -> ! {
        panic!("symbol type mismatch")
    }
    fn __pop_Variant1<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Block, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant1(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant2<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Data, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant2(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant3<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Dictionary, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant3(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant7<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Option<lexer::Token>, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant7(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant4<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Statement, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant4(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant6<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, Vec<Statement>, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant6(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant5<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, alloc::vec::Vec<Statement>, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant5(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant0<
    >(
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>
    ) -> (lexer::Location, lexer::Token, lexer::Location)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant0(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __reduce0<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Block = atom, sqs, Dictionary => ActionFn(29);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant3(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action29::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (3, 0)
    }
    fn __reduce1<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Block = atom, Dictionary => ActionFn(30);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant3(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action30::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (2, 0)
    }
    fn __reduce2<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Data = sqs => ActionFn(15);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action15::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (1, 1)
    }
    fn __reduce3<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Data = tqs => ActionFn(16);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action16::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (1, 1)
    }
    fn __reduce4<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Data = f64 => ActionFn(17);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action17::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (1, 1)
    }
    fn __reduce5<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Data = "$", atom => ActionFn(18);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action18::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (2, 1)
    }
    fn __reduce6<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Dictionary = "{", "}" => ActionFn(25);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action25::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant3(__nt), __end));
        (2, 2)
    }
    fn __reduce7<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Dictionary = "{", Statement+, "}" => ActionFn(26);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant0(__symbols);
        let __sym1 = __pop_Variant5(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action26::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant3(__nt), __end));
        (3, 2)
    }
    fn __reduce8<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = atom, "=", Data => ActionFn(6);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant2(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action6::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (3, 3)
    }
    fn __reduce9<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = Block => ActionFn(7);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action7::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (1, 3)
    }
    fn __reduce10<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = atom, ":", Data, "," => ActionFn(8);
        assert!(__symbols.len() >= 4);
        let __sym3 = __pop_Variant0(__symbols);
        let __sym2 = __pop_Variant2(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym3.2;
        let __nt = super::__action8::<>(__sym0, __sym1, __sym2, __sym3);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (4, 3)
    }
    fn __reduce11<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = Data, ":", Data, "," => ActionFn(9);
        assert!(__symbols.len() >= 4);
        let __sym3 = __pop_Variant0(__symbols);
        let __sym2 = __pop_Variant2(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant2(__symbols);
        let __start = __sym0.0;
        let __end = __sym3.2;
        let __nt = super::__action9::<>(__sym0, __sym1, __sym2, __sym3);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (4, 3)
    }
    fn __reduce12<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = atom, ":", Dictionary => ActionFn(10);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant3(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action10::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (3, 3)
    }
    fn __reduce13<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = Data, ":", Dictionary => ActionFn(11);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant3(__symbols);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant2(__symbols);
        let __start = __sym0.0;
        let __end = __sym2.2;
        let __nt = super::__action11::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (3, 3)
    }
    fn __reduce14<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = atom, "," => ActionFn(12);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant0(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action12::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (2, 3)
    }
    fn __reduce15<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement* =  => ActionFn(21);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action21::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant5(__nt), __end));
        (0, 4)
    }
    fn __reduce16<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement* = Statement+ => ActionFn(22);
        let __sym0 = __pop_Variant5(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action22::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant5(__nt), __end));
        (1, 4)
    }
    fn __reduce17<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement+ = Statement => ActionFn(23);
        let __sym0 = __pop_Variant4(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action23::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant5(__nt), __end));
        (1, 5)
    }
    fn __reduce18<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement+ = Statement+, Statement => ActionFn(24);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant4(__symbols);
        let __sym0 = __pop_Variant5(__symbols);
        let __start = __sym0.0;
        let __end = __sym1.2;
        let __nt = super::__action24::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant5(__nt), __end));
        (2, 5)
    }
    fn __reduce19<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statements =  => ActionFn(27);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action27::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant6(__nt), __end));
        (0, 6)
    }
    fn __reduce20<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statements = Statement+ => ActionFn(28);
        let __sym0 = __pop_Variant5(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action28::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant6(__nt), __end));
        (1, 6)
    }
    fn __reduce21<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // __Block = Block => ActionFn(2);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action2::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant1(__nt), __end));
        (1, 7)
    }
    fn __reduce22<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // __Data = Data => ActionFn(4);
        let __sym0 = __pop_Variant2(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action4::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant2(__nt), __end));
        (1, 8)
    }
    fn __reduce23<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // __Dictionary = Dictionary => ActionFn(3);
        let __sym0 = __pop_Variant3(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action3::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant3(__nt), __end));
        (1, 9)
    }
    fn __reduce24<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // __Statement = Statement => ActionFn(1);
        let __sym0 = __pop_Variant4(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action1::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant4(__nt), __end));
        (1, 10)
    }
    fn __reduce26<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // sqs? = sqs => ActionFn(19);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0;
        let __end = __sym0.2;
        let __nt = super::__action19::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant7(__nt), __end));
        (1, 12)
    }
    fn __reduce27<
    >(
        __lookahead_start: Option<&lexer::Location>,
        __symbols: &mut alloc::vec::Vec<(lexer::Location,__Symbol<>,lexer::Location)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // sqs? =  => ActionFn(20);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action20::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant7(__nt), __end));
        (0, 12)
    }
}
#[allow(unused_imports)]
pub use self::__parse__Statements::StatementsParser;

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action0<
>(
    (_, __0, _): (lexer::Location, Vec<Statement>, lexer::Location),
) -> Vec<Statement>
{
    __0
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action1<
>(
    (_, __0, _): (lexer::Location, Statement, lexer::Location),
) -> Statement
{
    __0
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action2<
>(
    (_, __0, _): (lexer::Location, Block, lexer::Location),
) -> Block
{
    __0
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action3<
>(
    (_, __0, _): (lexer::Location, Dictionary, lexer::Location),
) -> Dictionary
{
    __0
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action4<
>(
    (_, __0, _): (lexer::Location, Data, lexer::Location),
) -> Data
{
    __0
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action5<
>(
    (_, __0, _): (lexer::Location, alloc::vec::Vec<Statement>, lexer::Location),
) -> Vec<Statement>
{
    __0
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action6<
>(
    (_, a, _): (lexer::Location, lexer::Token, lexer::Location),
    (_, _, _): (lexer::Location, lexer::Token, lexer::Location),
    (_, d, _): (lexer::Location, Data, lexer::Location),
) -> Statement
{
    Statement::Assignment(a.try_into().unwrap(), d)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action7<
>(
    (_, b, _): (lexer::Location, Block, lexer::Location),
) -> Statement
{
    Statement::Block(b)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action8<
>(
    (_, a, _): (lexer::Location, lexer::Token, lexer::Location),
    (_, _, _): (lexer::Location, lexer::Token, lexer::Location),
    (_, d, _): (lexer::Location, Data, lexer::Location),
    (_, _, _): (lexer::Location, lexer::Token, lexer::Location),
) -> Statement
{
    Statement::AtomData(a.try_into().unwrap(), d)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action9<
>(
    (_, k, _): (lexer::Location, Data, lexer::Location),
    (_, _, _): (lexer::Location, lexer::Token, lexer::Location),
    (_, v, _): (lexer::Location, Data, lexer::Location),
    (_, _, _): (lexer::Location, lexer::Token, lexer::Location),
) -> Statement
{
    Statement::DataData(k, v)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action10<
>(
    (_, a, _): (lexer::Location, lexer::Token, lexer::Location),
    (_, _, _): (lexer::Location, lexer::Token, lexer::Location),
    (_, d, _): (lexer::Location, Dictionary, lexer::Location),
) -> Statement
{
    Statement::AtomDictionary(a.try_into().unwrap(), d)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action11<
>(
    (_, k, _): (lexer::Location, Data, lexer::Location),
    (_, _, _): (lexer::Location, lexer::Token, lexer::Location),
    (_, v, _): (lexer::Location, Dictionary, lexer::Location),
) -> Statement
{
    Statement::DataDictionary(k, v)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action12<
>(
    (_, a, _): (lexer::Location, lexer::Token, lexer::Location),
    (_, _, _): (lexer::Location, lexer::Token, lexer::Location),
) -> Statement
{
    Statement::Atom(a.try_into().unwrap())
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action13<
>(
    (_, a, _): (lexer::Location, lexer::Token, lexer::Location),
    (_, s, _): (lexer::Location, Option<lexer::Token>, lexer::Location),
    (_, d, _): (lexer::Location, Dictionary, lexer::Location),
) -> Block
{
    {
        Block {
            r#type: a.try_into().unwrap(),
            label: s.map(|s| s.try_into().unwrap()),
            dict: d,
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action14<
>(
    (_, _, _): (lexer::Location, lexer::Token, lexer::Location),
    (_, s, _): (lexer::Location, alloc::vec::Vec<Statement>, lexer::Location),
    (_, _, _): (lexer::Location, lexer::Token, lexer::Location),
) -> Dictionary
{
    {
        Dictionary {
            items: s,
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action15<
>(
    (_, x, _): (lexer::Location, lexer::Token, lexer::Location),
) -> Data
{
    x.try_into().unwrap()
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action16<
>(
    (_, x, _): (lexer::Location, lexer::Token, lexer::Location),
) -> Data
{
    x.try_into().unwrap()
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action17<
>(
    (_, x, _): (lexer::Location, lexer::Token, lexer::Location),
) -> Data
{
    x.try_into().unwrap()
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action18<
>(
    (_, _, _): (lexer::Location, lexer::Token, lexer::Location),
    (_, x, _): (lexer::Location, lexer::Token, lexer::Location),
) -> Data
{
    {
        match x {
            Token::Atom(x) => Data::Variable(x),
            _ => unreachable!(),
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action19<
>(
    (_, __0, _): (lexer::Location, lexer::Token, lexer::Location),
) -> Option<lexer::Token>
{
    Some(__0)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action20<
>(
    __lookbehind: &lexer::Location,
    __lookahead: &lexer::Location,
) -> Option<lexer::Token>
{
    None
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action21<
>(
    __lookbehind: &lexer::Location,
    __lookahead: &lexer::Location,
) -> alloc::vec::Vec<Statement>
{
    alloc::vec![]
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action22<
>(
    (_, v, _): (lexer::Location, alloc::vec::Vec<Statement>, lexer::Location),
) -> alloc::vec::Vec<Statement>
{
    v
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action23<
>(
    (_, __0, _): (lexer::Location, Statement, lexer::Location),
) -> alloc::vec::Vec<Statement>
{
    alloc::vec![__0]
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action24<
>(
    (_, v, _): (lexer::Location, alloc::vec::Vec<Statement>, lexer::Location),
    (_, e, _): (lexer::Location, Statement, lexer::Location),
) -> alloc::vec::Vec<Statement>
{
    { let mut v = v; v.push(e); v }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action25<
>(
    __0: (lexer::Location, lexer::Token, lexer::Location),
    __1: (lexer::Location, lexer::Token, lexer::Location),
) -> Dictionary
{
    let __start0 = __0.2;
    let __end0 = __1.0;
    let __temp0 = __action21(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action14(
        __0,
        __temp0,
        __1,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action26<
>(
    __0: (lexer::Location, lexer::Token, lexer::Location),
    __1: (lexer::Location, alloc::vec::Vec<Statement>, lexer::Location),
    __2: (lexer::Location, lexer::Token, lexer::Location),
) -> Dictionary
{
    let __start0 = __1.0;
    let __end0 = __1.2;
    let __temp0 = __action22(
        __1,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action14(
        __0,
        __temp0,
        __2,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action27<
>(
    __lookbehind: &lexer::Location,
    __lookahead: &lexer::Location,
) -> Vec<Statement>
{
    let __start0 = *__lookbehind;
    let __end0 = *__lookahead;
    let __temp0 = __action21(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action5(
        __temp0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action28<
>(
    __0: (lexer::Location, alloc::vec::Vec<Statement>, lexer::Location),
) -> Vec<Statement>
{
    let __start0 = __0.0;
    let __end0 = __0.2;
    let __temp0 = __action22(
        __0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action5(
        __temp0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action29<
>(
    __0: (lexer::Location, lexer::Token, lexer::Location),
    __1: (lexer::Location, lexer::Token, lexer::Location),
    __2: (lexer::Location, Dictionary, lexer::Location),
) -> Block
{
    let __start0 = __1.0;
    let __end0 = __1.2;
    let __temp0 = __action19(
        __1,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action13(
        __0,
        __temp0,
        __2,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action30<
>(
    __0: (lexer::Location, lexer::Token, lexer::Location),
    __1: (lexer::Location, Dictionary, lexer::Location),
) -> Block
{
    let __start0 = __0.2;
    let __end0 = __1.0;
    let __temp0 = __action20(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action13(
        __0,
        __temp0,
        __1,
    )
}

#[allow(clippy::type_complexity, dead_code)]
pub  trait __ToTriple<>
{
    fn to_triple(self) -> Result<(lexer::Location,lexer::Token,lexer::Location), __lalrpop_util::ParseError<lexer::Location, lexer::Token, lexer::LexicalError>>;
}

impl<> __ToTriple<> for (lexer::Location, lexer::Token, lexer::Location)
{
    fn to_triple(self) -> Result<(lexer::Location,lexer::Token,lexer::Location), __lalrpop_util::ParseError<lexer::Location, lexer::Token, lexer::LexicalError>> {
        Ok(self)
    }
}
impl<> __ToTriple<> for Result<(lexer::Location, lexer::Token, lexer::Location), lexer::LexicalError>
{
    fn to_triple(self) -> Result<(lexer::Location,lexer::Token,lexer::Location), __lalrpop_util::ParseError<lexer::Location, lexer::Token, lexer::LexicalError>> {
        self.map_err(|error| __lalrpop_util::ParseError::User { error })
    }
}
