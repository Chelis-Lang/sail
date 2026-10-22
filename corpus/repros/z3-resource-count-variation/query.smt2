(set-option :auto_config false)
(set-option :smt.mbqi false)
(set-option :smt.case_split 3)
(set-option :smt.qi.eager_threshold 100.0)
(set-option :smt.delay_units true)
(set-option :smt.arith.solver 2)
(set-option :smt.arith.nl false)
(set-option :pi.enabled false)
(set-option :rewriter.sort_disjunctions false)
(declare-sort %%Function%% 0)
(declare-sort FuelId 0)
(declare-sort Fuel 0)
(declare-const zero Fuel)
(declare-fun succ (Fuel) Fuel)
(declare-fun fuel_bool (FuelId) Bool)
(declare-fun fuel_bool_default (FuelId) Bool)
(declare-const fuel_defaults Bool)
(assert
 (=>
  fuel_defaults
  (forall ((id FuelId)) (!
    (= (fuel_bool id) (fuel_bool_default id))
    :pattern ((fuel_bool id))
    :qid prelude_fuel_defaults
    :skolemid skolem_prelude_fuel_defaults
))))
(declare-datatypes ((fndef 0)) (((fndef_singleton))))
(declare-sort Poly 0)
(declare-sort Height 0)
(declare-fun I (Int) Poly)
(declare-fun B (Bool) Poly)
(declare-fun R (Real) Poly)
(declare-fun F (fndef) Poly)
(declare-fun %I (Poly) Int)
(declare-fun %B (Poly) Bool)
(declare-fun %R (Poly) Real)
(declare-fun %F (Poly) fndef)
(declare-sort Type 0)
(declare-const BOOL Type)
(declare-const INT Type)
(declare-const NAT Type)
(declare-const REAL Type)
(declare-const CHAR Type)
(declare-const USIZE Type)
(declare-const ISIZE Type)
(declare-const TYPE%tuple%0. Type)
(declare-fun UINT (Int) Type)
(declare-fun SINT (Int) Type)
(declare-fun FLOAT (Int) Type)
(declare-fun CONST_INT (Int) Type)
(declare-fun CONST_BOOL (Bool) Type)
(declare-sort Dcr 0)
(declare-const $ Dcr)
(declare-const $slice Dcr)
(declare-const $dyn Dcr)
(declare-fun DST (Dcr) Dcr)
(declare-fun REF (Dcr) Dcr)
(declare-fun BOX (Dcr Type Dcr) Dcr)
(declare-fun RC (Dcr Type Dcr) Dcr)
(declare-fun ARC (Dcr Type Dcr) Dcr)
(declare-fun GHOST (Dcr) Dcr)
(declare-fun TRACKED (Dcr) Dcr)
(declare-fun NEVER (Dcr) Dcr)
(declare-fun CONST_PTR (Dcr) Dcr)
(declare-fun ARRAY (Dcr Type Dcr Type) Type)
(declare-fun MUTREF (Dcr Type) Type)
(declare-fun SLICE (Dcr Type) Type)
(declare-const STRSLICE Type)
(declare-const ALLOCATOR_GLOBAL Type)
(declare-fun PTR (Dcr Type) Type)
(declare-fun has_type (Poly Type) Bool)
(declare-fun sized (Dcr) Bool)
(declare-fun as_type (Poly Type) Poly)
(declare-fun mk_fun (Type %%Function%%) %%Function%%)
(declare-fun const_int (Type) Int)
(declare-fun const_bool (Type) Bool)
(declare-fun mut_ref_current% (Poly) Poly)
(declare-fun mut_ref_future% (Poly) Poly)
(declare-fun mut_ref_update_current% (Poly Poly) Poly)
(assert
 (forall ((m Poly) (arg Poly)) (!
   (= (mut_ref_current% (mut_ref_update_current% m arg)) arg)
   :pattern ((mut_ref_update_current% m arg))
   :qid prelude_mut_ref_update_current_current
   :skolemid skolem_prelude_mut_ref_update_current_current
)))
(assert
 (forall ((m Poly) (arg Poly)) (!
   (= (mut_ref_future% (mut_ref_update_current% m arg)) (mut_ref_future% m))
   :pattern ((mut_ref_update_current% m arg))
   :qid prelude_mut_ref_update_current_future
   :skolemid skolem_prelude_mut_ref_update_current_future
)))
(assert
 (forall ((m Poly) (d Dcr) (t Type)) (!
   (=>
    (has_type m (MUTREF d t))
    (has_type (mut_ref_current% m) t)
   )
   :pattern ((has_type m (MUTREF d t)) (mut_ref_current% m))
   :qid prelude_mut_ref_current_has_type
   :skolemid skolem_prelude_mut_ref_current_has_type
)))
(assert
 (forall ((m Poly) (d Dcr) (t Type)) (!
   (=>
    (has_type m (MUTREF d t))
    (has_type (mut_ref_future% m) t)
   )
   :pattern ((has_type m (MUTREF d t)) (mut_ref_future% m))
   :qid prelude_mut_ref_future_has_type
   :skolemid skolem_prelude_mut_ref_future_has_type
)))
(assert
 (forall ((m Poly) (d Dcr) (t Type) (arg Poly)) (!
   (=>
    (and
     (has_type m (MUTREF d t))
     (has_type arg t)
    )
    (has_type (mut_ref_update_current% m arg) (MUTREF d t))
   )
   :pattern ((has_type m (MUTREF d t)) (mut_ref_update_current% m arg))
   :qid prelude_mut_ref_update_has_type
   :skolemid skolem_prelude_mut_ref_update_has_type
)))
(assert
 (forall ((d Dcr)) (!
   (=>
    (sized d)
    (sized (DST d))
   )
   :pattern ((sized (DST d)))
   :qid prelude_sized_decorate_struct_inherit
   :skolemid skolem_prelude_sized_decorate_struct_inherit
)))
(assert
 (forall ((d Dcr)) (!
   (sized (REF d))
   :pattern ((sized (REF d)))
   :qid prelude_sized_decorate_ref
   :skolemid skolem_prelude_sized_decorate_ref
)))
(assert
 (forall ((d Dcr) (t Type) (d2 Dcr)) (!
   (sized (BOX d t d2))
   :pattern ((sized (BOX d t d2)))
   :qid prelude_sized_decorate_box
   :skolemid skolem_prelude_sized_decorate_box
)))
(assert
 (forall ((d Dcr) (t Type) (d2 Dcr)) (!
   (sized (RC d t d2))
   :pattern ((sized (RC d t d2)))
   :qid prelude_sized_decorate_rc
   :skolemid skolem_prelude_sized_decorate_rc
)))
(assert
 (forall ((d Dcr) (t Type) (d2 Dcr)) (!
   (sized (ARC d t d2))
   :pattern ((sized (ARC d t d2)))
   :qid prelude_sized_decorate_arc
   :skolemid skolem_prelude_sized_decorate_arc
)))
(assert
 (forall ((d Dcr)) (!
   (sized (GHOST d))
   :pattern ((sized (GHOST d)))
   :qid prelude_sized_decorate_ghost
   :skolemid skolem_prelude_sized_decorate_ghost
)))
(assert
 (forall ((d Dcr)) (!
   (sized (TRACKED d))
   :pattern ((sized (TRACKED d)))
   :qid prelude_sized_decorate_tracked
   :skolemid skolem_prelude_sized_decorate_tracked
)))
(assert
 (forall ((d Dcr)) (!
   (sized (NEVER d))
   :pattern ((sized (NEVER d)))
   :qid prelude_sized_decorate_never
   :skolemid skolem_prelude_sized_decorate_never
)))
(assert
 (forall ((d Dcr)) (!
   (sized (CONST_PTR d))
   :pattern ((sized (CONST_PTR d)))
   :qid prelude_sized_decorate_const_ptr
   :skolemid skolem_prelude_sized_decorate_const_ptr
)))
(assert
 (sized $)
)
(assert
 (forall ((i Int)) (!
   (= i (const_int (CONST_INT i)))
   :pattern ((CONST_INT i))
   :qid prelude_type_id_const_int
   :skolemid skolem_prelude_type_id_const_int
)))
(assert
 (forall ((b Bool)) (!
   (= b (const_bool (CONST_BOOL b)))
   :pattern ((CONST_BOOL b))
   :qid prelude_type_id_const_bool
   :skolemid skolem_prelude_type_id_const_bool
)))
(assert
 (forall ((b Bool)) (!
   (has_type (B b) BOOL)
   :pattern ((has_type (B b) BOOL))
   :qid prelude_has_type_bool
   :skolemid skolem_prelude_has_type_bool
)))
(assert
 (forall ((r Real)) (!
   (has_type (R r) REAL)
   :pattern ((has_type (R r) REAL))
   :qid prelude_has_type_real
   :skolemid skolem_prelude_has_type_real
)))
(assert
 (forall ((x Poly) (t Type)) (!
   (and
    (has_type (as_type x t) t)
    (=>
     (has_type x t)
     (= x (as_type x t))
   ))
   :pattern ((as_type x t))
   :qid prelude_as_type
   :skolemid skolem_prelude_as_type
)))
(assert
 (forall ((x Bool)) (!
   (= x (%B (B x)))
   :pattern ((B x))
   :qid prelude_unbox_box_bool
   :skolemid skolem_prelude_unbox_box_bool
)))
(assert
 (forall ((x Int)) (!
   (= x (%I (I x)))
   :pattern ((I x))
   :qid prelude_unbox_box_int
   :skolemid skolem_prelude_unbox_box_int
)))
(assert
 (forall ((x Real)) (!
   (= x (%R (R x)))
   :pattern ((R x))
   :qid prelude_unbox_box_real
   :skolemid skolem_prelude_unbox_box_real
)))
(assert
 (forall ((x Poly)) (!
   (=>
    (has_type x BOOL)
    (= x (B (%B x)))
   )
   :pattern ((has_type x BOOL))
   :qid prelude_box_unbox_bool
   :skolemid skolem_prelude_box_unbox_bool
)))
(assert
 (forall ((x Poly)) (!
   (=>
    (has_type x INT)
    (= x (I (%I x)))
   )
   :pattern ((has_type x INT))
   :qid prelude_box_unbox_int
   :skolemid skolem_prelude_box_unbox_int
)))
(assert
 (forall ((x Poly)) (!
   (=>
    (has_type x NAT)
    (= x (I (%I x)))
   )
   :pattern ((has_type x NAT))
   :qid prelude_box_unbox_nat
   :skolemid skolem_prelude_box_unbox_nat
)))
(assert
 (forall ((x Poly)) (!
   (=>
    (has_type x USIZE)
    (= x (I (%I x)))
   )
   :pattern ((has_type x USIZE))
   :qid prelude_box_unbox_usize
   :skolemid skolem_prelude_box_unbox_usize
)))
(assert
 (forall ((x Poly)) (!
   (=>
    (has_type x ISIZE)
    (= x (I (%I x)))
   )
   :pattern ((has_type x ISIZE))
   :qid prelude_box_unbox_isize
   :skolemid skolem_prelude_box_unbox_isize
)))
(assert
 (forall ((bits Int) (x Poly)) (!
   (=>
    (has_type x (UINT bits))
    (= x (I (%I x)))
   )
   :pattern ((has_type x (UINT bits)))
   :qid prelude_box_unbox_uint
   :skolemid skolem_prelude_box_unbox_uint
)))
(assert
 (forall ((bits Int) (x Poly)) (!
   (=>
    (has_type x (SINT bits))
    (= x (I (%I x)))
   )
   :pattern ((has_type x (SINT bits)))
   :qid prelude_box_unbox_sint
   :skolemid skolem_prelude_box_unbox_sint
)))
(assert
 (forall ((bits Int) (x Poly)) (!
   (=>
    (has_type x (FLOAT bits))
    (= x (I (%I x)))
   )
   :pattern ((has_type x (FLOAT bits)))
   :qid prelude_box_unbox_float
   :skolemid skolem_prelude_box_unbox_float
)))
(assert
 (forall ((x Poly)) (!
   (=>
    (has_type x CHAR)
    (= x (I (%I x)))
   )
   :pattern ((has_type x CHAR))
   :qid prelude_box_unbox_char
   :skolemid skolem_prelude_box_unbox_char
)))
(assert
 (forall ((x Poly)) (!
   (=>
    (has_type x REAL)
    (= x (R (%R x)))
   )
   :pattern ((has_type x REAL))
   :qid prelude_box_unbox_real
   :skolemid skolem_prelude_box_unbox_real
)))
(declare-fun ext_eq (Bool Type Poly Poly) Bool)
(assert
 (forall ((deep Bool) (t Type) (x Poly) (y Poly)) (!
   (= (= x y) (ext_eq deep t x y))
   :pattern ((ext_eq deep t x y))
   :qid prelude_ext_eq
   :skolemid skolem_prelude_ext_eq
)))
(declare-const SZ Int)
(assert
 (or
  (= SZ 32)
  (= SZ 64)
))
(declare-fun uHi (Int) Int)
(declare-fun iLo (Int) Int)
(declare-fun iHi (Int) Int)
(assert
 (= (uHi 8) 256)
)
(assert
 (= (uHi 16) 65536)
)
(assert
 (= (uHi 32) 4294967296)
)
(assert
 (= (uHi 64) 18446744073709551616)
)
(assert
 (= (uHi 128) (+ 1 340282366920938463463374607431768211455))
)
(assert
 (= (iLo 8) (- 128))
)
(assert
 (= (iLo 16) (- 32768))
)
(assert
 (= (iLo 32) (- 2147483648))
)
(assert
 (= (iLo 64) (- 9223372036854775808))
)
(assert
 (= (iLo 128) (- 170141183460469231731687303715884105728))
)
(assert
 (= (iHi 8) 128)
)
(assert
 (= (iHi 16) 32768)
)
(assert
 (= (iHi 32) 2147483648)
)
(assert
 (= (iHi 64) 9223372036854775808)
)
(assert
 (= (iHi 128) 170141183460469231731687303715884105728)
)
(declare-fun nClip (Int) Int)
(declare-fun uClip (Int Int) Int)
(declare-fun iClip (Int Int) Int)
(declare-fun charClip (Int) Int)
(assert
 (forall ((i Int)) (!
   (and
    (<= 0 (nClip i))
    (=>
     (<= 0 i)
     (= i (nClip i))
   ))
   :pattern ((nClip i))
   :qid prelude_nat_clip
   :skolemid skolem_prelude_nat_clip
)))
(assert
 (forall ((bits Int) (i Int)) (!
   (and
    (<= 0 (uClip bits i))
    (< (uClip bits i) (uHi bits))
    (=>
     (and
      (<= 0 i)
      (< i (uHi bits))
     )
     (= i (uClip bits i))
   ))
   :pattern ((uClip bits i))
   :qid prelude_u_clip
   :skolemid skolem_prelude_u_clip
)))
(assert
 (forall ((bits Int) (i Int)) (!
   (and
    (<= (iLo bits) (iClip bits i))
    (< (iClip bits i) (iHi bits))
    (=>
     (and
      (<= (iLo bits) i)
      (< i (iHi bits))
     )
     (= i (iClip bits i))
   ))
   :pattern ((iClip bits i))
   :qid prelude_i_clip
   :skolemid skolem_prelude_i_clip
)))
(assert
 (forall ((i Int)) (!
   (and
    (or
     (and
      (<= 0 (charClip i))
      (<= (charClip i) 55295)
     )
     (and
      (<= 57344 (charClip i))
      (<= (charClip i) 1114111)
    ))
    (=>
     (or
      (and
       (<= 0 i)
       (<= i 55295)
      )
      (and
       (<= 57344 i)
       (<= i 1114111)
     ))
     (= i (charClip i))
   ))
   :pattern ((charClip i))
   :qid prelude_char_clip
   :skolemid skolem_prelude_char_clip
)))
(declare-fun uInv (Int Int) Bool)
(declare-fun iInv (Int Int) Bool)
(declare-fun charInv (Int) Bool)
(assert
 (forall ((bits Int) (i Int)) (!
   (= (uInv bits i) (and
     (<= 0 i)
     (< i (uHi bits))
   ))
   :pattern ((uInv bits i))
   :qid prelude_u_inv
   :skolemid skolem_prelude_u_inv
)))
(assert
 (forall ((bits Int) (i Int)) (!
   (= (iInv bits i) (and
     (<= (iLo bits) i)
     (< i (iHi bits))
   ))
   :pattern ((iInv bits i))
   :qid prelude_i_inv
   :skolemid skolem_prelude_i_inv
)))
(assert
 (forall ((i Int)) (!
   (= (charInv i) (or
     (and
      (<= 0 i)
      (<= i 55295)
     )
     (and
      (<= 57344 i)
      (<= i 1114111)
   )))
   :pattern ((charInv i))
   :qid prelude_char_inv
   :skolemid skolem_prelude_char_inv
)))
(assert
 (forall ((x Int)) (!
   (has_type (I x) INT)
   :pattern ((has_type (I x) INT))
   :qid prelude_has_type_int
   :skolemid skolem_prelude_has_type_int
)))
(assert
 (forall ((x Int)) (!
   (=>
    (<= 0 x)
    (has_type (I x) NAT)
   )
   :pattern ((has_type (I x) NAT))
   :qid prelude_has_type_nat
   :skolemid skolem_prelude_has_type_nat
)))
(assert
 (forall ((x Int)) (!
   (=>
    (uInv SZ x)
    (has_type (I x) USIZE)
   )
   :pattern ((has_type (I x) USIZE))
   :qid prelude_has_type_usize
   :skolemid skolem_prelude_has_type_usize
)))
(assert
 (forall ((x Int)) (!
   (=>
    (iInv SZ x)
    (has_type (I x) ISIZE)
   )
   :pattern ((has_type (I x) ISIZE))
   :qid prelude_has_type_isize
   :skolemid skolem_prelude_has_type_isize
)))
(assert
 (forall ((bits Int) (x Int)) (!
   (=>
    (uInv bits x)
    (has_type (I x) (UINT bits))
   )
   :pattern ((has_type (I x) (UINT bits)))
   :qid prelude_has_type_uint
   :skolemid skolem_prelude_has_type_uint
)))
(assert
 (forall ((bits Int) (x Int)) (!
   (=>
    (iInv bits x)
    (has_type (I x) (SINT bits))
   )
   :pattern ((has_type (I x) (SINT bits)))
   :qid prelude_has_type_sint
   :skolemid skolem_prelude_has_type_sint
)))
(assert
 (forall ((bits Int) (x Int)) (!
   (=>
    (uInv bits x)
    (has_type (I x) (FLOAT bits))
   )
   :pattern ((has_type (I x) (FLOAT bits)))
   :qid prelude_has_type_float
   :skolemid skolem_prelude_has_type_float
)))
(assert
 (forall ((x Int)) (!
   (=>
    (charInv x)
    (has_type (I x) CHAR)
   )
   :pattern ((has_type (I x) CHAR))
   :qid prelude_has_type_char
   :skolemid skolem_prelude_has_type_char
)))
(assert
 (forall ((x Poly)) (!
   (=>
    (has_type x NAT)
    (<= 0 (%I x))
   )
   :pattern ((has_type x NAT))
   :qid prelude_unbox_int
   :skolemid skolem_prelude_unbox_int
)))
(assert
 (forall ((x Poly)) (!
   (=>
    (has_type x USIZE)
    (uInv SZ (%I x))
   )
   :pattern ((has_type x USIZE))
   :qid prelude_unbox_usize
   :skolemid skolem_prelude_unbox_usize
)))
(assert
 (forall ((x Poly)) (!
   (=>
    (has_type x CHAR)
    (charInv (%I x))
   )
   :pattern ((has_type x CHAR))
   :qid prelude_unbox_char
   :skolemid skolem_prelude_unbox_char
)))
(assert
 (forall ((x Poly)) (!
   (=>
    (has_type x ISIZE)
    (iInv SZ (%I x))
   )
   :pattern ((has_type x ISIZE))
   :qid prelude_unbox_isize
   :skolemid skolem_prelude_unbox_isize
)))
(assert
 (forall ((bits Int) (x Poly)) (!
   (=>
    (has_type x (UINT bits))
    (uInv bits (%I x))
   )
   :pattern ((has_type x (UINT bits)))
   :qid prelude_unbox_uint
   :skolemid skolem_prelude_unbox_uint
)))
(assert
 (forall ((bits Int) (x Poly)) (!
   (=>
    (has_type x (SINT bits))
    (iInv bits (%I x))
   )
   :pattern ((has_type x (SINT bits)))
   :qid prelude_unbox_sint
   :skolemid skolem_prelude_unbox_sint
)))
(assert
 (forall ((bits Int) (x Poly)) (!
   (=>
    (has_type x (FLOAT bits))
    (uInv bits (%I x))
   )
   :pattern ((has_type x (FLOAT bits)))
   :qid prelude_unbox_float
   :skolemid skolem_prelude_unbox_float
)))
(declare-fun Add (Int Int) Int)
(declare-fun Sub (Int Int) Int)
(declare-fun Mul (Int Int) Int)
(declare-fun EucDiv (Int Int) Int)
(declare-fun EucMod (Int Int) Int)
(declare-fun RAdd (Real Real) Real)
(declare-fun RSub (Real Real) Real)
(declare-fun RMul (Real Real) Real)
(declare-fun RDiv (Real Real) Real)
(assert
 (forall ((x Int) (y Int)) (!
   (= (Add x y) (+ x y))
   :pattern ((Add x y))
   :qid prelude_add
   :skolemid skolem_prelude_add
)))
(assert
 (forall ((x Int) (y Int)) (!
   (= (Sub x y) (- x y))
   :pattern ((Sub x y))
   :qid prelude_sub
   :skolemid skolem_prelude_sub
)))
(assert
 (forall ((x Int) (y Int)) (!
   (= (Mul x y) (* x y))
   :pattern ((Mul x y))
   :qid prelude_mul
   :skolemid skolem_prelude_mul
)))
(assert
 (forall ((x Int) (y Int)) (!
   (= (EucDiv x y) (div x y))
   :pattern ((EucDiv x y))
   :qid prelude_eucdiv
   :skolemid skolem_prelude_eucdiv
)))
(assert
 (forall ((x Int) (y Int)) (!
   (= (EucMod x y) (mod x y))
   :pattern ((EucMod x y))
   :qid prelude_eucmod
   :skolemid skolem_prelude_eucmod
)))
(assert
 (forall ((x Real) (y Real)) (!
   (= (RAdd x y) (+ x y))
   :pattern ((RAdd x y))
   :qid prelude_radd
   :skolemid skolem_prelude_radd
)))
(assert
 (forall ((x Real) (y Real)) (!
   (= (RSub x y) (- x y))
   :pattern ((RSub x y))
   :qid prelude_rsub
   :skolemid skolem_prelude_rsub
)))
(assert
 (forall ((x Real) (y Real)) (!
   (= (RMul x y) (* x y))
   :pattern ((RMul x y))
   :qid prelude_rmul
   :skolemid skolem_prelude_rmul
)))
(assert
 (forall ((x Real) (y Real)) (!
   (= (RDiv x y) (/ x y))
   :pattern ((RDiv x y))
   :qid prelude_rdiv
   :skolemid skolem_prelude_rdiv
)))
(assert
 (forall ((x Int) (y Int)) (!
   (=>
    (and
     (<= 0 x)
     (<= 0 y)
    )
    (<= 0 (Mul x y))
   )
   :pattern ((Mul x y))
   :qid prelude_mul_nats
   :skolemid skolem_prelude_mul_nats
)))
(assert
 (forall ((x Int) (y Int)) (!
   (=>
    (and
     (<= 0 x)
     (< 0 y)
    )
    (and
     (<= 0 (EucDiv x y))
     (<= (EucDiv x y) x)
   ))
   :pattern ((EucDiv x y))
   :qid prelude_div_unsigned_in_bounds
   :skolemid skolem_prelude_div_unsigned_in_bounds
)))
(assert
 (forall ((x Int) (y Int)) (!
   (=>
    (and
     (<= 0 x)
     (< 0 y)
    )
    (and
     (<= 0 (EucMod x y))
     (< (EucMod x y) y)
   ))
   :pattern ((EucMod x y))
   :qid prelude_mod_unsigned_in_bounds
   :skolemid skolem_prelude_mod_unsigned_in_bounds
)))
(declare-fun bitxor (Poly Poly) Int)
(declare-fun bitand (Poly Poly) Int)
(declare-fun bitor (Poly Poly) Int)
(declare-fun bitshr (Poly Poly) Int)
(declare-fun bitshl (Poly Poly) Int)
(declare-fun bitnot (Poly) Int)
(assert
 (forall ((x Poly)) (!
   (= (bitnot x) (- (- 0 (%I x)) 1))
   :pattern ((bitnot x))
   :qid prelude_bit_not
   :skolemid skolem_prelude_bit_not
)))
(assert
 (forall ((x Poly) (y Poly) (bits Int)) (!
   (=>
    (and
     (uInv bits (%I x))
     (uInv bits (%I y))
    )
    (uInv bits (bitxor x y))
   )
   :pattern ((uClip bits (bitxor x y)))
   :qid prelude_bit_xor_u_inv
   :skolemid skolem_prelude_bit_xor_u_inv
)))
(assert
 (forall ((x Poly) (y Poly) (bits Int)) (!
   (=>
    (and
     (iInv bits (%I x))
     (iInv bits (%I y))
    )
    (iInv bits (bitxor x y))
   )
   :pattern ((iClip bits (bitxor x y)))
   :qid prelude_bit_xor_i_inv
   :skolemid skolem_prelude_bit_xor_i_inv
)))
(assert
 (forall ((x Poly) (y Poly) (bits Int)) (!
   (=>
    (and
     (uInv bits (%I x))
     (uInv bits (%I y))
    )
    (uInv bits (bitor x y))
   )
   :pattern ((uClip bits (bitor x y)))
   :qid prelude_bit_or_u_inv
   :skolemid skolem_prelude_bit_or_u_inv
)))
(assert
 (forall ((x Poly) (y Poly) (bits Int)) (!
   (=>
    (and
     (iInv bits (%I x))
     (iInv bits (%I y))
    )
    (iInv bits (bitor x y))
   )
   :pattern ((iClip bits (bitor x y)))
   :qid prelude_bit_or_i_inv
   :skolemid skolem_prelude_bit_or_i_inv
)))
(assert
 (forall ((x Poly) (y Poly) (bits Int)) (!
   (=>
    (and
     (uInv bits (%I x))
     (uInv bits (%I y))
    )
    (uInv bits (bitand x y))
   )
   :pattern ((uClip bits (bitand x y)))
   :qid prelude_bit_and_u_inv
   :skolemid skolem_prelude_bit_and_u_inv
)))
(assert
 (forall ((x Poly) (y Poly) (bits Int)) (!
   (=>
    (and
     (iInv bits (%I x))
     (iInv bits (%I y))
    )
    (iInv bits (bitand x y))
   )
   :pattern ((iClip bits (bitand x y)))
   :qid prelude_bit_and_i_inv
   :skolemid skolem_prelude_bit_and_i_inv
)))
(assert
 (forall ((x Poly) (y Poly) (bits Int)) (!
   (=>
    (and
     (uInv bits (%I x))
     (<= 0 (%I y))
    )
    (uInv bits (bitshr x y))
   )
   :pattern ((uClip bits (bitshr x y)))
   :qid prelude_bit_shr_u_inv
   :skolemid skolem_prelude_bit_shr_u_inv
)))
(assert
 (forall ((x Poly) (y Poly) (bits Int)) (!
   (=>
    (and
     (iInv bits (%I x))
     (<= 0 (%I y))
    )
    (iInv bits (bitshr x y))
   )
   :pattern ((iClip bits (bitshr x y)))
   :qid prelude_bit_shr_i_inv
   :skolemid skolem_prelude_bit_shr_i_inv
)))
(declare-fun singular_mod (Int Int) Int)
(assert
 (forall ((x Int) (y Int)) (!
   (=>
    (not (= y 0))
    (= (EucMod x y) (singular_mod x y))
   )
   :pattern ((singular_mod x y))
   :qid prelude_singularmod
   :skolemid skolem_prelude_singularmod
)))
(declare-fun has_resolved (Dcr Type Poly) Bool)
(declare-fun closure_req (Type Dcr Type Poly Poly) Bool)
(declare-fun closure_ens (Type Dcr Type Poly Poly Poly) Bool)
(declare-fun default_ens (Type Dcr Type Poly Poly Poly) Bool)
(declare-fun height (Poly) Height)
(declare-fun height_lt (Height Height) Bool)
(declare-fun fun_from_recursive_field (Poly) Poly)
(declare-fun check_decrease_height (Poly Poly Bool) Bool)
(assert
 (forall ((cur Poly) (prev Poly) (otherwise Bool)) (!
   (= (check_decrease_height cur prev otherwise) (or
     (height_lt (height cur) (height prev))
     (and
      (= (height cur) (height prev))
      otherwise
   )))
   :pattern ((check_decrease_height cur prev otherwise))
   :qid prelude_check_decrease_height
   :skolemid skolem_prelude_check_decrease_height
)))
(assert
 (forall ((cur Int) (prev Int)) (!
   (= (height_lt (height (I cur)) (height (I prev))) (and
     (<= 0 cur)
     (< cur prev)
   ))
   :pattern ((height_lt (height (I cur)) (height (I prev))))
   :qid prelude_check_decrease_int_height
   :skolemid skolem_prelude_check_decrease_int_height
)))
(assert
 (forall ((x Height) (y Height)) (!
   (= (height_lt x y) (and
     ((_ partial-order 0) x y)
     (not (= x y))
   ))
   :pattern ((height_lt x y))
   :qid prelude_height_lt
   :skolemid skolem_prelude_height_lt
)))
(declare-const fuel%vstd!arithmetic.div_mod.lemma_div_basics_1. FuelId)
(declare-const fuel%vstd!arithmetic.div_mod.lemma_div_basics_2. FuelId)
(declare-const fuel%vstd!arithmetic.div_mod.lemma_div_basics_3. FuelId)
(declare-const fuel%vstd!arithmetic.div_mod.lemma_div_basics_4. FuelId)
(declare-const fuel%vstd!arithmetic.div_mod.lemma_div_basics_5. FuelId)
(declare-const fuel%vstd!arithmetic.div_mod.lemma_div_plus_one. FuelId)
(declare-const fuel%vstd!arithmetic.div_mod.lemma_fundamental_div_mod. FuelId)
(declare-const fuel%vstd!arithmetic.div_mod.lemma_mod_self_0. FuelId)
(declare-const fuel%vstd!arithmetic.div_mod.lemma_mod_is_zero. FuelId)
(declare-const fuel%vstd!arithmetic.div_mod.lemma_mod_multiples_basic. FuelId)
(declare-const fuel%vstd!arithmetic.div_mod.lemma_mod_sub_multiples_vanish. FuelId)
(declare-const fuel%vstd!arithmetic.div_mod.lemma_mod_multiples_vanish. FuelId)
(declare-const fuel%vstd!arithmetic.mul.lemma_mul_is_associative. FuelId)
(declare-const fuel%vstd!arithmetic.mul.lemma_mul_inequality. FuelId)
(declare-const fuel%vstd!arithmetic.mul.lemma_mul_is_distributive_add_other_way. FuelId)
(declare-const fuel%vstd!arithmetic.mul.lemma_mul_is_distributive_sub_other_way. FuelId)
(declare-const fuel%vstd!arithmetic.power.pow. FuelId)
(declare-const fuel%vstd!arithmetic.power.lemma_pow1. FuelId)
(declare-const fuel%vstd!arithmetic.power.lemma_pow_positive. FuelId)
(declare-const fuel%vstd!arithmetic.power.lemma_pow_adds. FuelId)
(declare-const fuel%vstd!arithmetic.power.lemma_pow_sub_add_cancel. FuelId)
(declare-const fuel%vstd!arithmetic.power.lemma_pow_multiplies. FuelId)
(declare-const fuel%vstd!arithmetic.power2.pow2. FuelId)
(declare-const fuel%vstd!arithmetic.power2.lemma_pow2. FuelId)
(declare-const fuel%vstd!endian.base_upper_bound_excl. FuelId)
(declare-const fuel%vstd!endian.impl&%0.in_bounds. FuelId)
(declare-const fuel%vstd!endian.impl&%0.wf. FuelId)
(declare-const fuel%vstd!endian.impl&%0.new. FuelId)
(declare-const fuel%vstd!endian.impl&%0.new_default. FuelId)
(declare-const fuel%vstd!endian.impl&%0.len. FuelId)
(declare-const fuel%vstd!endian.impl&%0.index. FuelId)
(declare-const fuel%vstd!endian.impl&%0.spec_index. FuelId)
(declare-const fuel%vstd!endian.impl&%0.first. FuelId)
(declare-const fuel%vstd!endian.impl&%0.last. FuelId)
(declare-const fuel%vstd!endian.impl&%0.least. FuelId)
(declare-const fuel%vstd!endian.impl&%0.most. FuelId)
(declare-const fuel%vstd!endian.impl&%0.skip. FuelId)
(declare-const fuel%vstd!endian.impl&%0.take. FuelId)
(declare-const fuel%vstd!endian.impl&%0.spec_index_range_from. FuelId)
(declare-const fuel%vstd!endian.impl&%0.spec_index_range_to. FuelId)
(declare-const fuel%vstd!endian.impl&%0.skip_least. FuelId)
(declare-const fuel%vstd!endian.impl&%0.skip_most. FuelId)
(declare-const fuel%vstd!endian.impl&%0.take_least. FuelId)
(declare-const fuel%vstd!endian.impl&%0.take_most. FuelId)
(declare-const fuel%vstd!endian.impl&%0.drop_first. FuelId)
(declare-const fuel%vstd!endian.impl&%0.drop_last. FuelId)
(declare-const fuel%vstd!endian.impl&%0.drop_least. FuelId)
(declare-const fuel%vstd!endian.impl&%0.drop_most. FuelId)
(declare-const fuel%vstd!endian.impl&%0.append. FuelId)
(declare-const fuel%vstd!endian.impl&%0.append_least. FuelId)
(declare-const fuel%vstd!endian.impl&%0.append_most. FuelId)
(declare-const fuel%vstd!endian.impl&%0.push_first. FuelId)
(declare-const fuel%vstd!endian.impl&%0.push_last. FuelId)
(declare-const fuel%vstd!endian.impl&%0.push_least. FuelId)
(declare-const fuel%vstd!endian.impl&%0.push_most. FuelId)
(declare-const fuel%vstd!endian.impl&%0.to_nat. FuelId)
(declare-const fuel%vstd!endian.impl&%0.to_nat_properties. FuelId)
(declare-const fuel%vstd!endian.impl&%0.from_nat. FuelId)
(declare-const fuel%vstd!endian.impl&%0.from_nat_properties. FuelId)
(declare-const fuel%vstd!endian.impl&%0.from_nat_to_nat. FuelId)
(declare-const fuel%vstd!endian.impl&%0.to_nat_from_nat. FuelId)
(declare-const fuel%vstd!endian.impl&%0.to_nat_injective. FuelId)
(declare-const fuel%vstd!endian.impl&%0.from_nat_injective. FuelId)
(declare-const fuel%vstd!endian.impl&%0.to_nat_most. FuelId)
(declare-const fuel%vstd!endian.impl&%0.to_nat_eq_to_nat_most. FuelId)
(declare-const fuel%vstd!endian.impl&%1.exp. FuelId)
(declare-const fuel%vstd!endian.impl&%1.exp_properties. FuelId)
(declare-const fuel%vstd!endian.impl&%1.from_big. FuelId)
(declare-const fuel%vstd!endian.impl&%1.from_big_properties. FuelId)
(declare-const fuel%vstd!endian.impl&%1.to_big. FuelId)
(declare-const fuel%vstd!endian.impl&%1.to_big_properties. FuelId)
(declare-const fuel%vstd!endian.impl&%1.to_big_from_big. FuelId)
(declare-const fuel%vstd!endian.impl&%1.from_big_to_big. FuelId)
(declare-const fuel%vstd!endian.impl&%1.to_big_injective. FuelId)
(declare-const fuel%vstd!endian.impl&%1.from_big_injective. FuelId)
(declare-const fuel%vstd!endian.to_big_from_digits. FuelId)
(declare-const fuel%vstd!endian.impl&%2.base. FuelId)
(declare-const fuel%vstd!endian.impl&%3.base. FuelId)
(declare-const fuel%vstd!endian.impl&%4.base. FuelId)
(declare-const fuel%vstd!endian.impl&%5.bits. FuelId)
(declare-const fuel%vstd!endian.impl&%6.bits. FuelId)
(declare-const fuel%vstd!endian.impl&%7.bits. FuelId)
(declare-const fuel%vstd!function.axiom_fn_mut_call_requires. FuelId)
(declare-const fuel%vstd!function.axiom_fn_mut_call_ensures. FuelId)
(declare-const fuel%vstd!seq.impl&%2.spec_index. FuelId)
(declare-const fuel%vstd!seq.impl&%2.spec_index_range_to. FuelId)
(declare-const fuel%vstd!seq.impl&%2.spec_index_range. FuelId)
(declare-const fuel%vstd!seq.impl&%2.take. FuelId)
(declare-const fuel%vstd!seq.impl&%2.skip. FuelId)
(declare-const fuel%vstd!seq.impl&%2.spec_add. FuelId)
(declare-const fuel%vstd!seq.Seq.last. FuelId)
(declare-const fuel%vstd!seq.impl&%2.first. FuelId)
(declare-const fuel%vstd!seq.lemma_seq_index_decreases. FuelId)
(declare-const fuel%vstd!seq.lemma_seq_subrange_decreases. FuelId)
(declare-const fuel%vstd!seq.lemma_seq_empty. FuelId)
(declare-const fuel%vstd!seq.lemma_seq_new_len. FuelId)
(declare-const fuel%vstd!seq.lemma_seq_new_index. FuelId)
(declare-const fuel%vstd!seq.lemma_seq_push_len. FuelId)
(declare-const fuel%vstd!seq.lemma_seq_push_index_same. FuelId)
(declare-const fuel%vstd!seq.lemma_seq_push_index_different. FuelId)
(declare-const fuel%vstd!seq.lemma_seq_ext_equal. FuelId)
(declare-const fuel%vstd!seq.lemma_seq_ext_equal_deep. FuelId)
(declare-const fuel%vstd!seq.lemma_seq_subrange_len. FuelId)
(declare-const fuel%vstd!seq.lemma_seq_subrange_index. FuelId)
(declare-const fuel%vstd!seq.lemma_seq_two_subranges_index. FuelId)
(declare-const fuel%vstd!seq.lemma_seq_add_len. FuelId)
(declare-const fuel%vstd!seq.lemma_seq_add_index1. FuelId)
(declare-const fuel%vstd!seq.lemma_seq_add_index2. FuelId)
(declare-const fuel%vstd!seq_lib.impl&%0.map. FuelId)
(declare-const fuel%vstd!seq_lib.impl&%0.add_empty_left. FuelId)
(declare-const fuel%vstd!seq_lib.impl&%0.add_empty_right. FuelId)
(declare-const fuel%vstd!seq_lib.impl&%0.push_distributes_over_add. FuelId)
(declare-const fuel%vstd!seq_lib.impl&%0.drop_last. FuelId)
(declare-const fuel%vstd!seq_lib.impl&%0.drop_first. FuelId)
(declare-const fuel%vstd!arithmetic.div_mod.group_div_basics. FuelId)
(declare-const fuel%vstd!array.group_array_axioms. FuelId)
(declare-const fuel%vstd!function.group_function_axioms. FuelId)
(declare-const fuel%vstd!imap.group_imap_lemmas. FuelId)
(declare-const fuel%vstd!iset.group_iset_lemmas. FuelId)
(declare-const fuel%vstd!laws_cmp.group_laws_cmp. FuelId)
(declare-const fuel%vstd!laws_eq.bool_laws.group_laws_eq. FuelId)
(declare-const fuel%vstd!laws_eq.u8_laws.group_laws_eq. FuelId)
(declare-const fuel%vstd!laws_eq.i8_laws.group_laws_eq. FuelId)
(declare-const fuel%vstd!laws_eq.u16_laws.group_laws_eq. FuelId)
(declare-const fuel%vstd!laws_eq.i16_laws.group_laws_eq. FuelId)
(declare-const fuel%vstd!laws_eq.u32_laws.group_laws_eq. FuelId)
(declare-const fuel%vstd!laws_eq.i32_laws.group_laws_eq. FuelId)
(declare-const fuel%vstd!laws_eq.u64_laws.group_laws_eq. FuelId)
(declare-const fuel%vstd!laws_eq.i64_laws.group_laws_eq. FuelId)
(declare-const fuel%vstd!laws_eq.u128_laws.group_laws_eq. FuelId)
(declare-const fuel%vstd!laws_eq.i128_laws.group_laws_eq. FuelId)
(declare-const fuel%vstd!laws_eq.usize_laws.group_laws_eq. FuelId)
(declare-const fuel%vstd!laws_eq.isize_laws.group_laws_eq. FuelId)
(declare-const fuel%vstd!laws_eq.tuple_1_laws.group_laws_eq. FuelId)
(declare-const fuel%vstd!laws_eq.tuple_2_laws.group_laws_eq. FuelId)
(declare-const fuel%vstd!laws_eq.tuple_3_laws.group_laws_eq. FuelId)
(declare-const fuel%vstd!laws_eq.tuple_4_laws.group_laws_eq. FuelId)
(declare-const fuel%vstd!laws_eq.tuple_5_laws.group_laws_eq. FuelId)
(declare-const fuel%vstd!laws_eq.tuple_6_laws.group_laws_eq. FuelId)
(declare-const fuel%vstd!laws_eq.tuple_7_laws.group_laws_eq. FuelId)
(declare-const fuel%vstd!laws_eq.tuple_8_laws.group_laws_eq. FuelId)
(declare-const fuel%vstd!laws_eq.tuple_9_laws.group_laws_eq. FuelId)
(declare-const fuel%vstd!laws_eq.tuple_10_laws.group_laws_eq. FuelId)
(declare-const fuel%vstd!laws_eq.tuple_11_laws.group_laws_eq. FuelId)
(declare-const fuel%vstd!laws_eq.tuple_12_laws.group_laws_eq. FuelId)
(declare-const fuel%vstd!laws_eq.group_laws_eq. FuelId)
(declare-const fuel%vstd!layout.group_align_properties. FuelId)
(declare-const fuel%vstd!layout.group_layout_axioms. FuelId)
(declare-const fuel%vstd!map.group_map_lemmas. FuelId)
(declare-const fuel%vstd!multiset.group_multiset_axioms. FuelId)
(declare-const fuel%vstd!mut_ref.group_mut_ref_axioms. FuelId)
(declare-const fuel%vstd!raw_ptr.group_raw_ptr_axioms. FuelId)
(declare-const fuel%vstd!seq.group_seq_lemmas. FuelId)
(declare-const fuel%vstd!seq_lib.group_filter_ensures. FuelId)
(declare-const fuel%vstd!seq_lib.group_seq_lib_default. FuelId)
(declare-const fuel%vstd!set.group_set_lemmas. FuelId)
(declare-const fuel%vstd!set_lib.group_set_lib_default. FuelId)
(declare-const fuel%vstd!slice.group_slice_axioms. FuelId)
(declare-const fuel%vstd!string.group_string_axioms. FuelId)
(declare-const fuel%vstd!std_specs.bits.group_bits_axioms. FuelId)
(declare-const fuel%vstd!std_specs.control_flow.group_control_flow_axioms. FuelId)
(declare-const fuel%vstd!std_specs.fmt.group_fmt_axioms. FuelId)
(declare-const fuel%vstd!std_specs.iter.group_iter_axioms. FuelId)
(declare-const fuel%vstd!std_specs.manually_drop.group_manually_drop_axioms. FuelId)
(declare-const fuel%vstd!std_specs.btree.group_btree_axioms. FuelId)
(declare-const fuel%vstd!std_specs.hash.group_hash_axioms. FuelId)
(declare-const fuel%vstd!std_specs.range.group_range_axioms. FuelId)
(declare-const fuel%vstd!std_specs.vec.group_vec_axioms. FuelId)
(declare-const fuel%vstd!std_specs.vecdeque.group_vec_dequeue_axioms. FuelId)
(declare-const fuel%vstd!group_vstd_default. FuelId)
(assert
 (distinct fuel%vstd!arithmetic.div_mod.lemma_div_basics_1. fuel%vstd!arithmetic.div_mod.lemma_div_basics_2.
  fuel%vstd!arithmetic.div_mod.lemma_div_basics_3. fuel%vstd!arithmetic.div_mod.lemma_div_basics_4.
  fuel%vstd!arithmetic.div_mod.lemma_div_basics_5. fuel%vstd!arithmetic.div_mod.lemma_div_plus_one.
  fuel%vstd!arithmetic.div_mod.lemma_fundamental_div_mod. fuel%vstd!arithmetic.div_mod.lemma_mod_self_0.
  fuel%vstd!arithmetic.div_mod.lemma_mod_is_zero. fuel%vstd!arithmetic.div_mod.lemma_mod_multiples_basic.
  fuel%vstd!arithmetic.div_mod.lemma_mod_sub_multiples_vanish. fuel%vstd!arithmetic.div_mod.lemma_mod_multiples_vanish.
  fuel%vstd!arithmetic.mul.lemma_mul_is_associative. fuel%vstd!arithmetic.mul.lemma_mul_inequality.
  fuel%vstd!arithmetic.mul.lemma_mul_is_distributive_add_other_way. fuel%vstd!arithmetic.mul.lemma_mul_is_distributive_sub_other_way.
  fuel%vstd!arithmetic.power.pow. fuel%vstd!arithmetic.power.lemma_pow1. fuel%vstd!arithmetic.power.lemma_pow_positive.
  fuel%vstd!arithmetic.power.lemma_pow_adds. fuel%vstd!arithmetic.power.lemma_pow_sub_add_cancel.
  fuel%vstd!arithmetic.power.lemma_pow_multiplies. fuel%vstd!arithmetic.power2.pow2.
  fuel%vstd!arithmetic.power2.lemma_pow2. fuel%vstd!endian.base_upper_bound_excl. fuel%vstd!endian.impl&%0.in_bounds.
  fuel%vstd!endian.impl&%0.wf. fuel%vstd!endian.impl&%0.new. fuel%vstd!endian.impl&%0.new_default.
  fuel%vstd!endian.impl&%0.len. fuel%vstd!endian.impl&%0.index. fuel%vstd!endian.impl&%0.spec_index.
  fuel%vstd!endian.impl&%0.first. fuel%vstd!endian.impl&%0.last. fuel%vstd!endian.impl&%0.least.
  fuel%vstd!endian.impl&%0.most. fuel%vstd!endian.impl&%0.skip. fuel%vstd!endian.impl&%0.take.
  fuel%vstd!endian.impl&%0.spec_index_range_from. fuel%vstd!endian.impl&%0.spec_index_range_to.
  fuel%vstd!endian.impl&%0.skip_least. fuel%vstd!endian.impl&%0.skip_most. fuel%vstd!endian.impl&%0.take_least.
  fuel%vstd!endian.impl&%0.take_most. fuel%vstd!endian.impl&%0.drop_first. fuel%vstd!endian.impl&%0.drop_last.
  fuel%vstd!endian.impl&%0.drop_least. fuel%vstd!endian.impl&%0.drop_most. fuel%vstd!endian.impl&%0.append.
  fuel%vstd!endian.impl&%0.append_least. fuel%vstd!endian.impl&%0.append_most. fuel%vstd!endian.impl&%0.push_first.
  fuel%vstd!endian.impl&%0.push_last. fuel%vstd!endian.impl&%0.push_least. fuel%vstd!endian.impl&%0.push_most.
  fuel%vstd!endian.impl&%0.to_nat. fuel%vstd!endian.impl&%0.to_nat_properties. fuel%vstd!endian.impl&%0.from_nat.
  fuel%vstd!endian.impl&%0.from_nat_properties. fuel%vstd!endian.impl&%0.from_nat_to_nat.
  fuel%vstd!endian.impl&%0.to_nat_from_nat. fuel%vstd!endian.impl&%0.to_nat_injective.
  fuel%vstd!endian.impl&%0.from_nat_injective. fuel%vstd!endian.impl&%0.to_nat_most.
  fuel%vstd!endian.impl&%0.to_nat_eq_to_nat_most. fuel%vstd!endian.impl&%1.exp. fuel%vstd!endian.impl&%1.exp_properties.
  fuel%vstd!endian.impl&%1.from_big. fuel%vstd!endian.impl&%1.from_big_properties.
  fuel%vstd!endian.impl&%1.to_big. fuel%vstd!endian.impl&%1.to_big_properties. fuel%vstd!endian.impl&%1.to_big_from_big.
  fuel%vstd!endian.impl&%1.from_big_to_big. fuel%vstd!endian.impl&%1.to_big_injective.
  fuel%vstd!endian.impl&%1.from_big_injective. fuel%vstd!endian.to_big_from_digits.
  fuel%vstd!endian.impl&%2.base. fuel%vstd!endian.impl&%3.base. fuel%vstd!endian.impl&%4.base.
  fuel%vstd!endian.impl&%5.bits. fuel%vstd!endian.impl&%6.bits. fuel%vstd!endian.impl&%7.bits.
  fuel%vstd!function.axiom_fn_mut_call_requires. fuel%vstd!function.axiom_fn_mut_call_ensures.
  fuel%vstd!seq.impl&%2.spec_index. fuel%vstd!seq.impl&%2.spec_index_range_to. fuel%vstd!seq.impl&%2.spec_index_range.
  fuel%vstd!seq.impl&%2.take. fuel%vstd!seq.impl&%2.skip. fuel%vstd!seq.impl&%2.spec_add.
  fuel%vstd!seq.Seq.last. fuel%vstd!seq.impl&%2.first. fuel%vstd!seq.lemma_seq_index_decreases.
  fuel%vstd!seq.lemma_seq_subrange_decreases. fuel%vstd!seq.lemma_seq_empty. fuel%vstd!seq.lemma_seq_new_len.
  fuel%vstd!seq.lemma_seq_new_index. fuel%vstd!seq.lemma_seq_push_len. fuel%vstd!seq.lemma_seq_push_index_same.
  fuel%vstd!seq.lemma_seq_push_index_different. fuel%vstd!seq.lemma_seq_ext_equal.
  fuel%vstd!seq.lemma_seq_ext_equal_deep. fuel%vstd!seq.lemma_seq_subrange_len. fuel%vstd!seq.lemma_seq_subrange_index.
  fuel%vstd!seq.lemma_seq_two_subranges_index. fuel%vstd!seq.lemma_seq_add_len. fuel%vstd!seq.lemma_seq_add_index1.
  fuel%vstd!seq.lemma_seq_add_index2. fuel%vstd!seq_lib.impl&%0.map. fuel%vstd!seq_lib.impl&%0.add_empty_left.
  fuel%vstd!seq_lib.impl&%0.add_empty_right. fuel%vstd!seq_lib.impl&%0.push_distributes_over_add.
  fuel%vstd!seq_lib.impl&%0.drop_last. fuel%vstd!seq_lib.impl&%0.drop_first. fuel%vstd!arithmetic.div_mod.group_div_basics.
  fuel%vstd!array.group_array_axioms. fuel%vstd!function.group_function_axioms. fuel%vstd!imap.group_imap_lemmas.
  fuel%vstd!iset.group_iset_lemmas. fuel%vstd!laws_cmp.group_laws_cmp. fuel%vstd!laws_eq.bool_laws.group_laws_eq.
  fuel%vstd!laws_eq.u8_laws.group_laws_eq. fuel%vstd!laws_eq.i8_laws.group_laws_eq.
  fuel%vstd!laws_eq.u16_laws.group_laws_eq. fuel%vstd!laws_eq.i16_laws.group_laws_eq.
  fuel%vstd!laws_eq.u32_laws.group_laws_eq. fuel%vstd!laws_eq.i32_laws.group_laws_eq.
  fuel%vstd!laws_eq.u64_laws.group_laws_eq. fuel%vstd!laws_eq.i64_laws.group_laws_eq.
  fuel%vstd!laws_eq.u128_laws.group_laws_eq. fuel%vstd!laws_eq.i128_laws.group_laws_eq.
  fuel%vstd!laws_eq.usize_laws.group_laws_eq. fuel%vstd!laws_eq.isize_laws.group_laws_eq.
  fuel%vstd!laws_eq.tuple_1_laws.group_laws_eq. fuel%vstd!laws_eq.tuple_2_laws.group_laws_eq.
  fuel%vstd!laws_eq.tuple_3_laws.group_laws_eq. fuel%vstd!laws_eq.tuple_4_laws.group_laws_eq.
  fuel%vstd!laws_eq.tuple_5_laws.group_laws_eq. fuel%vstd!laws_eq.tuple_6_laws.group_laws_eq.
  fuel%vstd!laws_eq.tuple_7_laws.group_laws_eq. fuel%vstd!laws_eq.tuple_8_laws.group_laws_eq.
  fuel%vstd!laws_eq.tuple_9_laws.group_laws_eq. fuel%vstd!laws_eq.tuple_10_laws.group_laws_eq.
  fuel%vstd!laws_eq.tuple_11_laws.group_laws_eq. fuel%vstd!laws_eq.tuple_12_laws.group_laws_eq.
  fuel%vstd!laws_eq.group_laws_eq. fuel%vstd!layout.group_align_properties. fuel%vstd!layout.group_layout_axioms.
  fuel%vstd!map.group_map_lemmas. fuel%vstd!multiset.group_multiset_axioms. fuel%vstd!mut_ref.group_mut_ref_axioms.
  fuel%vstd!raw_ptr.group_raw_ptr_axioms. fuel%vstd!seq.group_seq_lemmas. fuel%vstd!seq_lib.group_filter_ensures.
  fuel%vstd!seq_lib.group_seq_lib_default. fuel%vstd!set.group_set_lemmas. fuel%vstd!set_lib.group_set_lib_default.
  fuel%vstd!slice.group_slice_axioms. fuel%vstd!string.group_string_axioms. fuel%vstd!std_specs.bits.group_bits_axioms.
  fuel%vstd!std_specs.control_flow.group_control_flow_axioms. fuel%vstd!std_specs.fmt.group_fmt_axioms.
  fuel%vstd!std_specs.iter.group_iter_axioms. fuel%vstd!std_specs.manually_drop.group_manually_drop_axioms.
  fuel%vstd!std_specs.btree.group_btree_axioms. fuel%vstd!std_specs.hash.group_hash_axioms.
  fuel%vstd!std_specs.range.group_range_axioms. fuel%vstd!std_specs.vec.group_vec_axioms.
  fuel%vstd!std_specs.vecdeque.group_vec_dequeue_axioms. fuel%vstd!group_vstd_default.
))
(assert
 (=>
  (fuel_bool_default fuel%vstd!arithmetic.div_mod.group_div_basics.)
  (and
   (fuel_bool_default fuel%vstd!arithmetic.div_mod.lemma_div_basics_1.)
   (fuel_bool_default fuel%vstd!arithmetic.div_mod.lemma_div_basics_2.)
   (fuel_bool_default fuel%vstd!arithmetic.div_mod.lemma_div_basics_3.)
   (fuel_bool_default fuel%vstd!arithmetic.div_mod.lemma_div_basics_4.)
   (fuel_bool_default fuel%vstd!arithmetic.div_mod.lemma_div_basics_5.)
)))
(assert
 (=>
  (fuel_bool_default fuel%vstd!function.group_function_axioms.)
  (and
   (fuel_bool_default fuel%vstd!function.axiom_fn_mut_call_requires.)
   (fuel_bool_default fuel%vstd!function.axiom_fn_mut_call_ensures.)
)))
(assert
 (=>
  (fuel_bool_default fuel%vstd!laws_eq.group_laws_eq.)
  (and
   (fuel_bool_default fuel%vstd!laws_eq.bool_laws.group_laws_eq.)
   (fuel_bool_default fuel%vstd!laws_eq.u8_laws.group_laws_eq.)
   (fuel_bool_default fuel%vstd!laws_eq.i8_laws.group_laws_eq.)
   (fuel_bool_default fuel%vstd!laws_eq.u16_laws.group_laws_eq.)
   (fuel_bool_default fuel%vstd!laws_eq.i16_laws.group_laws_eq.)
   (fuel_bool_default fuel%vstd!laws_eq.u32_laws.group_laws_eq.)
   (fuel_bool_default fuel%vstd!laws_eq.i32_laws.group_laws_eq.)
   (fuel_bool_default fuel%vstd!laws_eq.u64_laws.group_laws_eq.)
   (fuel_bool_default fuel%vstd!laws_eq.i64_laws.group_laws_eq.)
   (fuel_bool_default fuel%vstd!laws_eq.u128_laws.group_laws_eq.)
   (fuel_bool_default fuel%vstd!laws_eq.i128_laws.group_laws_eq.)
   (fuel_bool_default fuel%vstd!laws_eq.usize_laws.group_laws_eq.)
   (fuel_bool_default fuel%vstd!laws_eq.isize_laws.group_laws_eq.)
   (fuel_bool_default fuel%vstd!laws_eq.tuple_1_laws.group_laws_eq.)
   (fuel_bool_default fuel%vstd!laws_eq.tuple_2_laws.group_laws_eq.)
   (fuel_bool_default fuel%vstd!laws_eq.tuple_3_laws.group_laws_eq.)
   (fuel_bool_default fuel%vstd!laws_eq.tuple_4_laws.group_laws_eq.)
   (fuel_bool_default fuel%vstd!laws_eq.tuple_5_laws.group_laws_eq.)
   (fuel_bool_default fuel%vstd!laws_eq.tuple_6_laws.group_laws_eq.)
   (fuel_bool_default fuel%vstd!laws_eq.tuple_7_laws.group_laws_eq.)
   (fuel_bool_default fuel%vstd!laws_eq.tuple_8_laws.group_laws_eq.)
   (fuel_bool_default fuel%vstd!laws_eq.tuple_9_laws.group_laws_eq.)
   (fuel_bool_default fuel%vstd!laws_eq.tuple_10_laws.group_laws_eq.)
   (fuel_bool_default fuel%vstd!laws_eq.tuple_11_laws.group_laws_eq.)
   (fuel_bool_default fuel%vstd!laws_eq.tuple_12_laws.group_laws_eq.)
)))
(assert
 (=>
  (fuel_bool_default fuel%vstd!layout.group_layout_axioms.)
  (fuel_bool_default fuel%vstd!layout.group_align_properties.)
))
(assert
 (=>
  (fuel_bool_default fuel%vstd!seq.group_seq_lemmas.)
  (and
   (fuel_bool_default fuel%vstd!seq.lemma_seq_index_decreases.)
   (fuel_bool_default fuel%vstd!seq.lemma_seq_subrange_decreases.)
   (fuel_bool_default fuel%vstd!seq.lemma_seq_empty.)
   (fuel_bool_default fuel%vstd!seq.lemma_seq_new_len.)
   (fuel_bool_default fuel%vstd!seq.lemma_seq_new_index.)
   (fuel_bool_default fuel%vstd!seq.lemma_seq_push_len.)
   (fuel_bool_default fuel%vstd!seq.lemma_seq_push_index_same.)
   (fuel_bool_default fuel%vstd!seq.lemma_seq_push_index_different.)
   (fuel_bool_default fuel%vstd!seq.lemma_seq_ext_equal.)
   (fuel_bool_default fuel%vstd!seq.lemma_seq_ext_equal_deep.)
   (fuel_bool_default fuel%vstd!seq.lemma_seq_subrange_len.)
   (fuel_bool_default fuel%vstd!seq.lemma_seq_subrange_index.)
   (fuel_bool_default fuel%vstd!seq.lemma_seq_two_subranges_index.)
   (fuel_bool_default fuel%vstd!seq.lemma_seq_add_len.)
   (fuel_bool_default fuel%vstd!seq.lemma_seq_add_index1.)
   (fuel_bool_default fuel%vstd!seq.lemma_seq_add_index2.)
)))
(assert
 (=>
  (fuel_bool_default fuel%vstd!seq_lib.group_seq_lib_default.)
  (and
   (fuel_bool_default fuel%vstd!seq_lib.group_filter_ensures.)
   (fuel_bool_default fuel%vstd!seq_lib.impl&%0.add_empty_left.)
   (fuel_bool_default fuel%vstd!seq_lib.impl&%0.add_empty_right.)
   (fuel_bool_default fuel%vstd!seq_lib.impl&%0.push_distributes_over_add.)
)))
(assert
 (=>
  (fuel_bool_default fuel%vstd!group_vstd_default.)
  (and
   (fuel_bool_default fuel%vstd!seq.group_seq_lemmas.)
   (fuel_bool_default fuel%vstd!seq_lib.group_seq_lib_default.)
   (fuel_bool_default fuel%vstd!map.group_map_lemmas.)
   (fuel_bool_default fuel%vstd!set.group_set_lemmas.)
   (fuel_bool_default fuel%vstd!imap.group_imap_lemmas.)
   (fuel_bool_default fuel%vstd!iset.group_iset_lemmas.)
   (fuel_bool_default fuel%vstd!set_lib.group_set_lib_default.)
   (fuel_bool_default fuel%vstd!multiset.group_multiset_axioms.)
   (fuel_bool_default fuel%vstd!function.group_function_axioms.)
   (fuel_bool_default fuel%vstd!laws_eq.group_laws_eq.)
   (fuel_bool_default fuel%vstd!laws_cmp.group_laws_cmp.)
   (fuel_bool_default fuel%vstd!slice.group_slice_axioms.)
   (fuel_bool_default fuel%vstd!array.group_array_axioms.)
   (fuel_bool_default fuel%vstd!string.group_string_axioms.)
   (fuel_bool_default fuel%vstd!raw_ptr.group_raw_ptr_axioms.)
   (fuel_bool_default fuel%vstd!layout.group_layout_axioms.)
   (fuel_bool_default fuel%vstd!mut_ref.group_mut_ref_axioms.)
   (fuel_bool_default fuel%vstd!std_specs.range.group_range_axioms.)
   (fuel_bool_default fuel%vstd!std_specs.bits.group_bits_axioms.)
   (fuel_bool_default fuel%vstd!std_specs.control_flow.group_control_flow_axioms.)
   (fuel_bool_default fuel%vstd!std_specs.fmt.group_fmt_axioms.)
   (fuel_bool_default fuel%vstd!std_specs.manually_drop.group_manually_drop_axioms.)
   (fuel_bool_default fuel%vstd!std_specs.iter.group_iter_axioms.)
   (fuel_bool_default fuel%vstd!std_specs.vec.group_vec_axioms.)
   (fuel_bool_default fuel%vstd!std_specs.vecdeque.group_vec_dequeue_axioms.)
   (fuel_bool_default fuel%vstd!std_specs.hash.group_hash_axioms.)
   (fuel_bool_default fuel%vstd!std_specs.btree.group_btree_axioms.)
)))
(assert
 (fuel_bool_default fuel%vstd!group_vstd_default.)
)
(declare-fun tr_bound%vstd!endian.Base. (Dcr Type) Bool)
(declare-fun tr_bound%vstd!endian.BasePow2. (Dcr Type) Bool)
(declare-fun tr_bound%vstd!endian.CompatibleSmallerBaseFor. (Dcr Type Dcr Type) Bool)
(declare-fun tr_bound%core!clone.Clone. (Dcr Type) Bool)
(declare-fun tr_bound%core!marker.Copy. (Dcr Type) Bool)
(declare-fun tr_bound%core!marker.Tuple. (Dcr Type) Bool)
(declare-fun tr_bound%core!ops.function.FnOnce. (Dcr Type Dcr Type) Bool)
(declare-fun tr_bound%core!ops.function.FnMut. (Dcr Type Dcr Type) Bool)
(declare-fun tr_bound%core!ops.function.Fn. (Dcr Type Dcr Type) Bool)
(declare-fun tr_bound%verus_builtin!Integer. (Dcr Type) Bool)
(declare-fun tr_bound%core!alloc.Allocator. (Dcr Type) Bool)
(declare-fun proj%%core!ops.function.FnOnce./Output (Dcr Type Dcr Type) Dcr)
(declare-fun proj%core!ops.function.FnOnce./Output (Dcr Type Dcr Type) Type)
(declare-sort vstd!seq.Seq<int.>. 0)
(declare-datatypes ((core!marker.PhantomData. 0) (vstd!endian.Endian. 0) (vstd!endian.EndianNat.
   0
  ) (tuple%0. 0)
 ) (((core!marker.PhantomData./PhantomData)) ((vstd!endian.Endian./Little) (vstd!endian.Endian./Big))
  ((vstd!endian.EndianNat./EndianNat (vstd!endian.EndianNat./EndianNat/?endian vstd!endian.Endian.)
    (vstd!endian.EndianNat./EndianNat/?digits vstd!seq.Seq<int.>.) (vstd!endian.EndianNat./EndianNat/?phantom
     core!marker.PhantomData.
   ))
  ) ((tuple%0./tuple%0))
))
(declare-fun vstd!endian.EndianNat./EndianNat/endian (vstd!endian.EndianNat.) vstd!endian.Endian.)
(declare-fun vstd!endian.EndianNat./EndianNat/digits (vstd!endian.EndianNat.) vstd!seq.Seq<int.>.)
(declare-fun vstd!endian.EndianNat./EndianNat/phantom (vstd!endian.EndianNat.) core!marker.PhantomData.)
(declare-fun TYPE%fun%1. (Dcr Type Dcr Type) Type)
(declare-fun TYPE%fun%2. (Dcr Type Dcr Type Dcr Type) Type)
(declare-fun TYPE%core!marker.PhantomData. (Dcr Type) Type)
(declare-const TYPE%vstd!endian.Endian. Type)
(declare-fun TYPE%vstd!endian.EndianNat. (Dcr Type) Type)
(declare-fun TYPE%vstd!seq.Seq. (Dcr Type) Type)
(declare-fun Poly%fun%1. (%%Function%%) Poly)
(declare-fun %Poly%fun%1. (Poly) %%Function%%)
(declare-fun Poly%fun%2. (%%Function%%) Poly)
(declare-fun %Poly%fun%2. (Poly) %%Function%%)
(declare-fun Poly%vstd!seq.Seq<int.>. (vstd!seq.Seq<int.>.) Poly)
(declare-fun %Poly%vstd!seq.Seq<int.>. (Poly) vstd!seq.Seq<int.>.)
(declare-fun Poly%core!marker.PhantomData. (core!marker.PhantomData.) Poly)
(declare-fun %Poly%core!marker.PhantomData. (Poly) core!marker.PhantomData.)
(declare-fun Poly%vstd!endian.Endian. (vstd!endian.Endian.) Poly)
(declare-fun %Poly%vstd!endian.Endian. (Poly) vstd!endian.Endian.)
(declare-fun Poly%vstd!endian.EndianNat. (vstd!endian.EndianNat.) Poly)
(declare-fun %Poly%vstd!endian.EndianNat. (Poly) vstd!endian.EndianNat.)
(declare-fun Poly%tuple%0. (tuple%0.) Poly)
(declare-fun %Poly%tuple%0. (Poly) tuple%0.)
(assert
 (forall ((x %%Function%%)) (!
   (= x (%Poly%fun%1. (Poly%fun%1. x)))
   :pattern ((Poly%fun%1. x))
   :qid internal_crate__fun__1_box_axiom_definition
   :skolemid skolem_internal_crate__fun__1_box_axiom_definition
)))
(assert
 (forall ((T%0&. Dcr) (T%0& Type) (T%1&. Dcr) (T%1& Type) (x Poly)) (!
   (=>
    (has_type x (TYPE%fun%1. T%0&. T%0& T%1&. T%1&))
    (= x (Poly%fun%1. (%Poly%fun%1. x)))
   )
   :pattern ((has_type x (TYPE%fun%1. T%0&. T%0& T%1&. T%1&)))
   :qid internal_crate__fun__1_unbox_axiom_definition
   :skolemid skolem_internal_crate__fun__1_unbox_axiom_definition
)))
(declare-fun %%apply%%0 (%%Function%% Poly) Poly)
(assert
 (forall ((T%0&. Dcr) (T%0& Type) (T%1&. Dcr) (T%1& Type) (x %%Function%%)) (!
   (=>
    (forall ((T%0 Poly)) (!
      (=>
       (has_type T%0 T%0&)
       (has_type (%%apply%%0 (mk_fun (TYPE%fun%1. T%0&. T%0& T%1&. T%1&) x) T%0) T%1&)
      )
      :pattern ((has_type (%%apply%%0 (mk_fun (TYPE%fun%1. T%0&. T%0& T%1&. T%1&) x) T%0)
        T%1&
      ))
      :qid internal_crate__fun__1_constructor_inner_definition
      :skolemid skolem_internal_crate__fun__1_constructor_inner_definition
    ))
    (has_type (Poly%fun%1. (mk_fun (TYPE%fun%1. T%0&. T%0& T%1&. T%1&) x)) (TYPE%fun%1.
      T%0&. T%0& T%1&. T%1&
   )))
   :pattern ((has_type (Poly%fun%1. (mk_fun (TYPE%fun%1. T%0&. T%0& T%1&. T%1&) x)) (TYPE%fun%1.
      T%0&. T%0& T%1&. T%1&
   )))
   :qid internal_crate__fun__1_constructor_definition
   :skolemid skolem_internal_crate__fun__1_constructor_definition
)))
(assert
 (forall ((T%0&. Dcr) (T%0& Type) (T%1&. Dcr) (T%1& Type) (T%0 Poly) (x %%Function%%))
  (!
   (=>
    (and
     (has_type (Poly%fun%1. x) (TYPE%fun%1. T%0&. T%0& T%1&. T%1&))
     (has_type T%0 T%0&)
    )
    (has_type (%%apply%%0 x T%0) T%1&)
   )
   :pattern ((%%apply%%0 x T%0) (has_type (Poly%fun%1. x) (TYPE%fun%1. T%0&. T%0& T%1&.
      T%1&
   )))
   :qid internal_crate__fun__1_apply_definition
   :skolemid skolem_internal_crate__fun__1_apply_definition
)))
(assert
 (forall ((T%0&. Dcr) (T%0& Type) (T%1&. Dcr) (T%1& Type) (T%0 Poly) (x %%Function%%))
  (!
   (=>
    (and
     (has_type (Poly%fun%1. x) (TYPE%fun%1. T%0&. T%0& T%1&. T%1&))
     (has_type T%0 T%0&)
    )
    (height_lt (height (%%apply%%0 x T%0)) (height (fun_from_recursive_field (Poly%fun%1.
        x
   )))))
   :pattern ((height (%%apply%%0 x T%0)) (has_type (Poly%fun%1. x) (TYPE%fun%1. T%0&. T%0&
      T%1&. T%1&
   )))
   :qid internal_crate__fun__1_height_apply_definition
   :skolemid skolem_internal_crate__fun__1_height_apply_definition
)))
(assert
 (forall ((T%0&. Dcr) (T%0& Type) (T%1&. Dcr) (T%1& Type) (deep Bool) (x Poly) (y Poly))
  (!
   (=>
    (and
     (has_type x (TYPE%fun%1. T%0&. T%0& T%1&. T%1&))
     (has_type y (TYPE%fun%1. T%0&. T%0& T%1&. T%1&))
     (forall ((T%0 Poly)) (!
       (=>
        (has_type T%0 T%0&)
        (ext_eq deep T%1& (%%apply%%0 (%Poly%fun%1. x) T%0) (%%apply%%0 (%Poly%fun%1. y) T%0))
       )
       :pattern ((ext_eq deep T%1& (%%apply%%0 (%Poly%fun%1. x) T%0) (%%apply%%0 (%Poly%fun%1.
           y
          ) T%0
       )))
       :qid internal_crate__fun__1_inner_ext_equal_definition
       :skolemid skolem_internal_crate__fun__1_inner_ext_equal_definition
    )))
    (ext_eq deep (TYPE%fun%1. T%0&. T%0& T%1&. T%1&) x y)
   )
   :pattern ((ext_eq deep (TYPE%fun%1. T%0&. T%0& T%1&. T%1&) x y))
   :qid internal_crate__fun__1_ext_equal_definition
   :skolemid skolem_internal_crate__fun__1_ext_equal_definition
)))
(assert
 (forall ((x %%Function%%)) (!
   (= x (%Poly%fun%2. (Poly%fun%2. x)))
   :pattern ((Poly%fun%2. x))
   :qid internal_crate__fun__2_box_axiom_definition
   :skolemid skolem_internal_crate__fun__2_box_axiom_definition
)))
(assert
 (forall ((T%0&. Dcr) (T%0& Type) (T%1&. Dcr) (T%1& Type) (T%2&. Dcr) (T%2& Type) (x
    Poly
   )
  ) (!
   (=>
    (has_type x (TYPE%fun%2. T%0&. T%0& T%1&. T%1& T%2&. T%2&))
    (= x (Poly%fun%2. (%Poly%fun%2. x)))
   )
   :pattern ((has_type x (TYPE%fun%2. T%0&. T%0& T%1&. T%1& T%2&. T%2&)))
   :qid internal_crate__fun__2_unbox_axiom_definition
   :skolemid skolem_internal_crate__fun__2_unbox_axiom_definition
)))
(declare-fun %%apply%%1 (%%Function%% Poly Poly) Poly)
(assert
 (forall ((T%0&. Dcr) (T%0& Type) (T%1&. Dcr) (T%1& Type) (T%2&. Dcr) (T%2& Type) (x
    %%Function%%
   )
  ) (!
   (=>
    (forall ((T%0 Poly) (T%1 Poly)) (!
      (=>
       (and
        (has_type T%0 T%0&)
        (has_type T%1 T%1&)
       )
       (has_type (%%apply%%1 (mk_fun (TYPE%fun%2. T%0&. T%0& T%1&. T%1& T%2&. T%2&) x) T%0
         T%1
        ) T%2&
      ))
      :pattern ((has_type (%%apply%%1 (mk_fun (TYPE%fun%2. T%0&. T%0& T%1&. T%1& T%2&. T%2&)
          x
         ) T%0 T%1
        ) T%2&
      ))
      :qid internal_crate__fun__2_constructor_inner_definition
      :skolemid skolem_internal_crate__fun__2_constructor_inner_definition
    ))
    (has_type (Poly%fun%2. (mk_fun (TYPE%fun%2. T%0&. T%0& T%1&. T%1& T%2&. T%2&) x))
     (TYPE%fun%2. T%0&. T%0& T%1&. T%1& T%2&. T%2&)
   ))
   :pattern ((has_type (Poly%fun%2. (mk_fun (TYPE%fun%2. T%0&. T%0& T%1&. T%1& T%2&. T%2&)
       x
      )
     ) (TYPE%fun%2. T%0&. T%0& T%1&. T%1& T%2&. T%2&)
   ))
   :qid internal_crate__fun__2_constructor_definition
   :skolemid skolem_internal_crate__fun__2_constructor_definition
)))
(assert
 (forall ((T%0&. Dcr) (T%0& Type) (T%1&. Dcr) (T%1& Type) (T%2&. Dcr) (T%2& Type) (T%0
    Poly
   ) (T%1 Poly) (x %%Function%%)
  ) (!
   (=>
    (and
     (has_type (Poly%fun%2. x) (TYPE%fun%2. T%0&. T%0& T%1&. T%1& T%2&. T%2&))
     (has_type T%0 T%0&)
     (has_type T%1 T%1&)
    )
    (has_type (%%apply%%1 x T%0 T%1) T%2&)
   )
   :pattern ((%%apply%%1 x T%0 T%1) (has_type (Poly%fun%2. x) (TYPE%fun%2. T%0&. T%0& T%1&.
      T%1& T%2&. T%2&
   )))
   :qid internal_crate__fun__2_apply_definition
   :skolemid skolem_internal_crate__fun__2_apply_definition
)))
(assert
 (forall ((T%0&. Dcr) (T%0& Type) (T%1&. Dcr) (T%1& Type) (T%2&. Dcr) (T%2& Type) (T%0
    Poly
   ) (T%1 Poly) (x %%Function%%)
  ) (!
   (=>
    (and
     (has_type (Poly%fun%2. x) (TYPE%fun%2. T%0&. T%0& T%1&. T%1& T%2&. T%2&))
     (has_type T%0 T%0&)
     (has_type T%1 T%1&)
    )
    (height_lt (height (%%apply%%1 x T%0 T%1)) (height (fun_from_recursive_field (Poly%fun%2.
        x
   )))))
   :pattern ((height (%%apply%%1 x T%0 T%1)) (has_type (Poly%fun%2. x) (TYPE%fun%2. T%0&.
      T%0& T%1&. T%1& T%2&. T%2&
   )))
   :qid internal_crate__fun__2_height_apply_definition
   :skolemid skolem_internal_crate__fun__2_height_apply_definition
)))
(assert
 (forall ((T%0&. Dcr) (T%0& Type) (T%1&. Dcr) (T%1& Type) (T%2&. Dcr) (T%2& Type) (deep
    Bool
   ) (x Poly) (y Poly)
  ) (!
   (=>
    (and
     (has_type x (TYPE%fun%2. T%0&. T%0& T%1&. T%1& T%2&. T%2&))
     (has_type y (TYPE%fun%2. T%0&. T%0& T%1&. T%1& T%2&. T%2&))
     (forall ((T%0 Poly) (T%1 Poly)) (!
       (=>
        (and
         (has_type T%0 T%0&)
         (has_type T%1 T%1&)
        )
        (ext_eq deep T%2& (%%apply%%1 (%Poly%fun%2. x) T%0 T%1) (%%apply%%1 (%Poly%fun%2. y)
          T%0 T%1
       )))
       :pattern ((ext_eq deep T%2& (%%apply%%1 (%Poly%fun%2. x) T%0 T%1) (%%apply%%1 (%Poly%fun%2.
           y
          ) T%0 T%1
       )))
       :qid internal_crate__fun__2_inner_ext_equal_definition
       :skolemid skolem_internal_crate__fun__2_inner_ext_equal_definition
    )))
    (ext_eq deep (TYPE%fun%2. T%0&. T%0& T%1&. T%1& T%2&. T%2&) x y)
   )
   :pattern ((ext_eq deep (TYPE%fun%2. T%0&. T%0& T%1&. T%1& T%2&. T%2&) x y))
   :qid internal_crate__fun__2_ext_equal_definition
   :skolemid skolem_internal_crate__fun__2_ext_equal_definition
)))
(assert
 (forall ((x vstd!seq.Seq<int.>.)) (!
   (= x (%Poly%vstd!seq.Seq<int.>. (Poly%vstd!seq.Seq<int.>. x)))
   :pattern ((Poly%vstd!seq.Seq<int.>. x))
   :qid internal_vstd__seq__Seq<int.>_box_axiom_definition
   :skolemid skolem_internal_vstd__seq__Seq<int.>_box_axiom_definition
)))
(assert
 (forall ((x Poly)) (!
   (=>
    (has_type x (TYPE%vstd!seq.Seq. $ INT))
    (= x (Poly%vstd!seq.Seq<int.>. (%Poly%vstd!seq.Seq<int.>. x)))
   )
   :pattern ((has_type x (TYPE%vstd!seq.Seq. $ INT)))
   :qid internal_vstd__seq__Seq<int.>_unbox_axiom_definition
   :skolemid skolem_internal_vstd__seq__Seq<int.>_unbox_axiom_definition
)))
(assert
 (forall ((x vstd!seq.Seq<int.>.)) (!
   (has_type (Poly%vstd!seq.Seq<int.>. x) (TYPE%vstd!seq.Seq. $ INT))
   :pattern ((has_type (Poly%vstd!seq.Seq<int.>. x) (TYPE%vstd!seq.Seq. $ INT)))
   :qid internal_vstd__seq__Seq<int.>_has_type_always_definition
   :skolemid skolem_internal_vstd__seq__Seq<int.>_has_type_always_definition
)))
(assert
 (forall ((x core!marker.PhantomData.)) (!
   (= x (%Poly%core!marker.PhantomData. (Poly%core!marker.PhantomData. x)))
   :pattern ((Poly%core!marker.PhantomData. x))
   :qid internal_core__marker__PhantomData_box_axiom_definition
   :skolemid skolem_internal_core__marker__PhantomData_box_axiom_definition
)))
(assert
 (forall ((V&. Dcr) (V& Type) (x Poly)) (!
   (=>
    (has_type x (TYPE%core!marker.PhantomData. V&. V&))
    (= x (Poly%core!marker.PhantomData. (%Poly%core!marker.PhantomData. x)))
   )
   :pattern ((has_type x (TYPE%core!marker.PhantomData. V&. V&)))
   :qid internal_core__marker__PhantomData_unbox_axiom_definition
   :skolemid skolem_internal_core__marker__PhantomData_unbox_axiom_definition
)))
(assert
 (forall ((V&. Dcr) (V& Type) (x core!marker.PhantomData.)) (!
   (has_type (Poly%core!marker.PhantomData. x) (TYPE%core!marker.PhantomData. V&. V&))
   :pattern ((has_type (Poly%core!marker.PhantomData. x) (TYPE%core!marker.PhantomData.
      V&. V&
   )))
   :qid internal_core__marker__PhantomData_has_type_always_definition
   :skolemid skolem_internal_core__marker__PhantomData_has_type_always_definition
)))
(assert
 (forall ((x vstd!endian.Endian.)) (!
   (= x (%Poly%vstd!endian.Endian. (Poly%vstd!endian.Endian. x)))
   :pattern ((Poly%vstd!endian.Endian. x))
   :qid internal_vstd__endian__Endian_box_axiom_definition
   :skolemid skolem_internal_vstd__endian__Endian_box_axiom_definition
)))
(assert
 (forall ((x Poly)) (!
   (=>
    (has_type x TYPE%vstd!endian.Endian.)
    (= x (Poly%vstd!endian.Endian. (%Poly%vstd!endian.Endian. x)))
   )
   :pattern ((has_type x TYPE%vstd!endian.Endian.))
   :qid internal_vstd__endian__Endian_unbox_axiom_definition
   :skolemid skolem_internal_vstd__endian__Endian_unbox_axiom_definition
)))
(assert
 (forall ((x vstd!endian.Endian.)) (!
   (has_type (Poly%vstd!endian.Endian. x) TYPE%vstd!endian.Endian.)
   :pattern ((has_type (Poly%vstd!endian.Endian. x) TYPE%vstd!endian.Endian.))
   :qid internal_vstd__endian__Endian_has_type_always_definition
   :skolemid skolem_internal_vstd__endian__Endian_has_type_always_definition
)))
(assert
 (forall ((x vstd!endian.EndianNat.)) (!
   (= x (%Poly%vstd!endian.EndianNat. (Poly%vstd!endian.EndianNat. x)))
   :pattern ((Poly%vstd!endian.EndianNat. x))
   :qid internal_vstd__endian__EndianNat_box_axiom_definition
   :skolemid skolem_internal_vstd__endian__EndianNat_box_axiom_definition
)))
(assert
 (forall ((B&. Dcr) (B& Type) (x Poly)) (!
   (=>
    (has_type x (TYPE%vstd!endian.EndianNat. B&. B&))
    (= x (Poly%vstd!endian.EndianNat. (%Poly%vstd!endian.EndianNat. x)))
   )
   :pattern ((has_type x (TYPE%vstd!endian.EndianNat. B&. B&)))
   :qid internal_vstd__endian__EndianNat_unbox_axiom_definition
   :skolemid skolem_internal_vstd__endian__EndianNat_unbox_axiom_definition
)))
(assert
 (forall ((x vstd!endian.EndianNat.)) (!
   (= (vstd!endian.EndianNat./EndianNat/endian x) (vstd!endian.EndianNat./EndianNat/?endian
     x
   ))
   :pattern ((vstd!endian.EndianNat./EndianNat/endian x))
   :qid internal_vstd!endian.EndianNat./EndianNat/endian_accessor_definition
   :skolemid skolem_internal_vstd!endian.EndianNat./EndianNat/endian_accessor_definition
)))
(assert
 (forall ((x vstd!endian.EndianNat.)) (!
   (= (vstd!endian.EndianNat./EndianNat/digits x) (vstd!endian.EndianNat./EndianNat/?digits
     x
   ))
   :pattern ((vstd!endian.EndianNat./EndianNat/digits x))
   :qid internal_vstd!endian.EndianNat./EndianNat/digits_accessor_definition
   :skolemid skolem_internal_vstd!endian.EndianNat./EndianNat/digits_accessor_definition
)))
(assert
 (forall ((x vstd!endian.EndianNat.)) (!
   (= (vstd!endian.EndianNat./EndianNat/phantom x) (vstd!endian.EndianNat./EndianNat/?phantom
     x
   ))
   :pattern ((vstd!endian.EndianNat./EndianNat/phantom x))
   :qid internal_vstd!endian.EndianNat./EndianNat/phantom_accessor_definition
   :skolemid skolem_internal_vstd!endian.EndianNat./EndianNat/phantom_accessor_definition
)))
(assert
 (forall ((B&. Dcr) (B& Type) (x vstd!endian.EndianNat.)) (!
   (has_type (Poly%vstd!endian.EndianNat. x) (TYPE%vstd!endian.EndianNat. B&. B&))
   :pattern ((has_type (Poly%vstd!endian.EndianNat. x) (TYPE%vstd!endian.EndianNat. B&.
      B&
   )))
   :qid internal_vstd__endian__EndianNat_has_type_always_definition
   :skolemid skolem_internal_vstd__endian__EndianNat_has_type_always_definition
)))
(assert
 (forall ((x vstd!endian.EndianNat.)) (!
   (=>
    (is-vstd!endian.EndianNat./EndianNat x)
    (height_lt (height (Poly%core!marker.PhantomData. (vstd!endian.EndianNat./EndianNat/phantom
        x
      ))
     ) (height (Poly%vstd!endian.EndianNat. x))
   ))
   :pattern ((height (Poly%core!marker.PhantomData. (vstd!endian.EndianNat./EndianNat/phantom
       x
   ))))
   :qid prelude_datatype_height_vstd!endian.EndianNat./EndianNat/phantom
   :skolemid skolem_prelude_datatype_height_vstd!endian.EndianNat./EndianNat/phantom
)))
(assert
 (forall ((B&. Dcr) (B& Type) (deep Bool) (x Poly) (y Poly)) (!
   (=>
    (and
     (has_type x (TYPE%vstd!endian.EndianNat. B&. B&))
     (has_type y (TYPE%vstd!endian.EndianNat. B&. B&))
     (= (vstd!endian.EndianNat./EndianNat/endian (%Poly%vstd!endian.EndianNat. x)) (vstd!endian.EndianNat./EndianNat/endian
       (%Poly%vstd!endian.EndianNat. y)
     ))
     (ext_eq deep (TYPE%vstd!seq.Seq. $ INT) (Poly%vstd!seq.Seq<int.>. (vstd!endian.EndianNat./EndianNat/digits
        (%Poly%vstd!endian.EndianNat. x)
       )
      ) (Poly%vstd!seq.Seq<int.>. (vstd!endian.EndianNat./EndianNat/digits (%Poly%vstd!endian.EndianNat.
         y
     ))))
     (= (vstd!endian.EndianNat./EndianNat/phantom (%Poly%vstd!endian.EndianNat. x)) (vstd!endian.EndianNat./EndianNat/phantom
       (%Poly%vstd!endian.EndianNat. y)
    )))
    (ext_eq deep (TYPE%vstd!endian.EndianNat. B&. B&) x y)
   )
   :pattern ((ext_eq deep (TYPE%vstd!endian.EndianNat. B&. B&) x y))
   :qid internal_vstd!endian.EndianNat./EndianNat_ext_equal_definition
   :skolemid skolem_internal_vstd!endian.EndianNat./EndianNat_ext_equal_definition
)))
(assert
 (forall ((x tuple%0.)) (!
   (= x (%Poly%tuple%0. (Poly%tuple%0. x)))
   :pattern ((Poly%tuple%0. x))
   :qid internal_crate__tuple__0_box_axiom_definition
   :skolemid skolem_internal_crate__tuple__0_box_axiom_definition
)))
(assert
 (forall ((x Poly)) (!
   (=>
    (has_type x TYPE%tuple%0.)
    (= x (Poly%tuple%0. (%Poly%tuple%0. x)))
   )
   :pattern ((has_type x TYPE%tuple%0.))
   :qid internal_crate__tuple__0_unbox_axiom_definition
   :skolemid skolem_internal_crate__tuple__0_unbox_axiom_definition
)))
(assert
 (forall ((x tuple%0.)) (!
   (has_type (Poly%tuple%0. x) TYPE%tuple%0.)
   :pattern ((has_type (Poly%tuple%0. x) TYPE%tuple%0.))
   :qid internal_crate__tuple__0_has_type_always_definition
   :skolemid skolem_internal_crate__tuple__0_has_type_always_definition
)))
(assert
 (forall ((Self%&. Dcr) (Self%& Type)) (!
   true
   :pattern ((tr_bound%vstd!endian.Base. Self%&. Self%&))
   :qid internal_vstd__endian__Base_trait_type_bounds_definition
   :skolemid skolem_internal_vstd__endian__Base_trait_type_bounds_definition
)))
(assert
 (forall ((Self%&. Dcr) (Self%& Type)) (!
   (=>
    (tr_bound%vstd!endian.BasePow2. Self%&. Self%&)
    (tr_bound%vstd!endian.Base. Self%&. Self%&)
   )
   :pattern ((tr_bound%vstd!endian.BasePow2. Self%&. Self%&))
   :qid internal_vstd__endian__BasePow2_trait_type_bounds_definition
   :skolemid skolem_internal_vstd__endian__BasePow2_trait_type_bounds_definition
)))
(assert
 (forall ((Self%&. Dcr) (Self%& Type) (BIG&. Dcr) (BIG& Type)) (!
   (=>
    (tr_bound%vstd!endian.CompatibleSmallerBaseFor. Self%&. Self%& BIG&. BIG&)
    (and
     (tr_bound%vstd!endian.BasePow2. Self%&. Self%&)
     (sized BIG&.)
     (tr_bound%vstd!endian.BasePow2. BIG&. BIG&)
   ))
   :pattern ((tr_bound%vstd!endian.CompatibleSmallerBaseFor. Self%&. Self%& BIG&. BIG&))
   :qid internal_vstd__endian__CompatibleSmallerBaseFor_trait_type_bounds_definition
   :skolemid skolem_internal_vstd__endian__CompatibleSmallerBaseFor_trait_type_bounds_definition
)))
(assert
 (forall ((Self%&. Dcr) (Self%& Type)) (!
   (=>
    (tr_bound%core!clone.Clone. Self%&. Self%&)
    (sized Self%&.)
   )
   :pattern ((tr_bound%core!clone.Clone. Self%&. Self%&))
   :qid internal_core__clone__Clone_trait_type_bounds_definition
   :skolemid skolem_internal_core__clone__Clone_trait_type_bounds_definition
)))
(assert
 (forall ((Self%&. Dcr) (Self%& Type)) (!
   (=>
    (tr_bound%core!marker.Copy. Self%&. Self%&)
    (tr_bound%core!clone.Clone. Self%&. Self%&)
   )
   :pattern ((tr_bound%core!marker.Copy. Self%&. Self%&))
   :qid internal_core__marker__Copy_trait_type_bounds_definition
   :skolemid skolem_internal_core__marker__Copy_trait_type_bounds_definition
)))
(assert
 (forall ((Self%&. Dcr) (Self%& Type)) (!
   true
   :pattern ((tr_bound%core!marker.Tuple. Self%&. Self%&))
   :qid internal_core__marker__Tuple_trait_type_bounds_definition
   :skolemid skolem_internal_core__marker__Tuple_trait_type_bounds_definition
)))
(assert
 (forall ((Self%&. Dcr) (Self%& Type) (Args&. Dcr) (Args& Type)) (!
   (=>
    (tr_bound%core!ops.function.FnOnce. Self%&. Self%& Args&. Args&)
    (and
     (sized Args&.)
     (tr_bound%core!marker.Tuple. Args&. Args&)
     (sized (proj%%core!ops.function.FnOnce./Output Self%&. Self%& Args&. Args&))
   ))
   :pattern ((tr_bound%core!ops.function.FnOnce. Self%&. Self%& Args&. Args&))
   :qid internal_core__ops__function__FnOnce_trait_type_bounds_definition
   :skolemid skolem_internal_core__ops__function__FnOnce_trait_type_bounds_definition
)))
(assert
 (forall ((Self%&. Dcr) (Self%& Type) (Args&. Dcr) (Args& Type)) (!
   (=>
    (tr_bound%core!ops.function.FnMut. Self%&. Self%& Args&. Args&)
    (and
     (tr_bound%core!ops.function.FnOnce. Self%&. Self%& Args&. Args&)
     (sized Args&.)
     (tr_bound%core!marker.Tuple. Args&. Args&)
   ))
   :pattern ((tr_bound%core!ops.function.FnMut. Self%&. Self%& Args&. Args&))
   :qid internal_core__ops__function__FnMut_trait_type_bounds_definition
   :skolemid skolem_internal_core__ops__function__FnMut_trait_type_bounds_definition
)))
(assert
 (forall ((Self%&. Dcr) (Self%& Type) (Args&. Dcr) (Args& Type)) (!
   (=>
    (tr_bound%core!ops.function.Fn. Self%&. Self%& Args&. Args&)
    (and
     (tr_bound%core!ops.function.FnMut. Self%&. Self%& Args&. Args&)
     (sized Args&.)
     (tr_bound%core!marker.Tuple. Args&. Args&)
   ))
   :pattern ((tr_bound%core!ops.function.Fn. Self%&. Self%& Args&. Args&))
   :qid internal_core__ops__function__Fn_trait_type_bounds_definition
   :skolemid skolem_internal_core__ops__function__Fn_trait_type_bounds_definition
)))
(assert
 (forall ((Self%&. Dcr) (Self%& Type)) (!
   (=>
    (tr_bound%verus_builtin!Integer. Self%&. Self%&)
    (tr_bound%core!marker.Copy. Self%&. Self%&)
   )
   :pattern ((tr_bound%verus_builtin!Integer. Self%&. Self%&))
   :qid internal_verus_builtin__Integer_trait_type_bounds_definition
   :skolemid skolem_internal_verus_builtin__Integer_trait_type_bounds_definition
)))
(assert
 (forall ((Self%&. Dcr) (Self%& Type)) (!
   true
   :pattern ((tr_bound%core!alloc.Allocator. Self%&. Self%&))
   :qid internal_core__alloc__Allocator_trait_type_bounds_definition
   :skolemid skolem_internal_core__alloc__Allocator_trait_type_bounds_definition
)))
(assert
 (forall ((A&. Dcr) (A& Type) (F&. Dcr) (F& Type)) (!
   (=>
    (and
     (sized A&.)
     (tr_bound%core!marker.Tuple. A&. A&)
     (tr_bound%core!ops.function.Fn. F&. F& A&. A&)
    )
    (= (proj%%core!ops.function.FnOnce./Output (REF F&.) F& A&. A&) (proj%%core!ops.function.FnOnce./Output
      F&. F& A&. A&
   )))
   :pattern ((proj%%core!ops.function.FnOnce./Output (REF F&.) F& A&. A&))
   :qid internal_proj____core!ops.function.FnOnce./Output_core__ops__function__impls__impl&__2_assoc_type_impl_true_definition
   :skolemid skolem_internal_proj____core!ops.function.FnOnce./Output_core__ops__function__impls__impl&__2_assoc_type_impl_true_definition
)))
(assert
 (forall ((A&. Dcr) (A& Type) (F&. Dcr) (F& Type)) (!
   (=>
    (and
     (sized A&.)
     (tr_bound%core!marker.Tuple. A&. A&)
     (tr_bound%core!ops.function.Fn. F&. F& A&. A&)
    )
    (= (proj%core!ops.function.FnOnce./Output (REF F&.) F& A&. A&) (proj%core!ops.function.FnOnce./Output
      F&. F& A&. A&
   )))
   :pattern ((proj%core!ops.function.FnOnce./Output (REF F&.) F& A&. A&))
   :qid internal_proj__core!ops.function.FnOnce./Output_core__ops__function__impls__impl&__2_assoc_type_impl_false_definition
   :skolemid skolem_internal_proj__core!ops.function.FnOnce./Output_core__ops__function__impls__impl&__2_assoc_type_impl_false_definition
)))
(assert
 (forall ((A&. Dcr) (A& Type) (F&. Dcr) (F& Type)) (!
   (=>
    (and
     (sized A&.)
     (tr_bound%core!marker.Tuple. A&. A&)
     (tr_bound%core!ops.function.FnMut. F&. F& A&. A&)
    )
    (= (proj%%core!ops.function.FnOnce./Output $ (MUTREF F&. F&) A&. A&) (proj%%core!ops.function.FnOnce./Output
      F&. F& A&. A&
   )))
   :pattern ((proj%%core!ops.function.FnOnce./Output $ (MUTREF F&. F&) A&. A&))
   :qid internal_proj____core!ops.function.FnOnce./Output_core__ops__function__impls__impl&__4_assoc_type_impl_true_definition
   :skolemid skolem_internal_proj____core!ops.function.FnOnce./Output_core__ops__function__impls__impl&__4_assoc_type_impl_true_definition
)))
(assert
 (forall ((A&. Dcr) (A& Type) (F&. Dcr) (F& Type)) (!
   (=>
    (and
     (sized A&.)
     (tr_bound%core!marker.Tuple. A&. A&)
     (tr_bound%core!ops.function.FnMut. F&. F& A&. A&)
    )
    (= (proj%core!ops.function.FnOnce./Output $ (MUTREF F&. F&) A&. A&) (proj%core!ops.function.FnOnce./Output
      F&. F& A&. A&
   )))
   :pattern ((proj%core!ops.function.FnOnce./Output $ (MUTREF F&. F&) A&. A&))
   :qid internal_proj__core!ops.function.FnOnce./Output_core__ops__function__impls__impl&__4_assoc_type_impl_false_definition
   :skolemid skolem_internal_proj__core!ops.function.FnOnce./Output_core__ops__function__impls__impl&__4_assoc_type_impl_false_definition
)))
(assert
 (forall ((Args&. Dcr) (Args& Type) (F&. Dcr) (F& Type) (A&. Dcr) (A& Type)) (!
   (=>
    (and
     (sized Args&.)
     (sized A&.)
     (tr_bound%core!marker.Tuple. Args&. Args&)
     (tr_bound%core!ops.function.FnOnce. F&. F& Args&. Args&)
     (tr_bound%core!alloc.Allocator. A&. A&)
    )
    (= (proj%%core!ops.function.FnOnce./Output (BOX A&. A& F&.) F& Args&. Args&) (proj%%core!ops.function.FnOnce./Output
      F&. F& Args&. Args&
   )))
   :pattern ((proj%%core!ops.function.FnOnce./Output (BOX A&. A& F&.) F& Args&. Args&))
   :qid internal_proj____core!ops.function.FnOnce./Output_alloc__boxed__impl&__31_assoc_type_impl_true_definition
   :skolemid skolem_internal_proj____core!ops.function.FnOnce./Output_alloc__boxed__impl&__31_assoc_type_impl_true_definition
)))
(assert
 (forall ((Args&. Dcr) (Args& Type) (F&. Dcr) (F& Type) (A&. Dcr) (A& Type)) (!
   (=>
    (and
     (sized Args&.)
     (sized A&.)
     (tr_bound%core!marker.Tuple. Args&. Args&)
     (tr_bound%core!ops.function.FnOnce. F&. F& Args&. Args&)
     (tr_bound%core!alloc.Allocator. A&. A&)
    )
    (= (proj%core!ops.function.FnOnce./Output (BOX A&. A& F&.) F& Args&. Args&) (proj%core!ops.function.FnOnce./Output
      F&. F& Args&. Args&
   )))
   :pattern ((proj%core!ops.function.FnOnce./Output (BOX A&. A& F&.) F& Args&. Args&))
   :qid internal_proj__core!ops.function.FnOnce./Output_alloc__boxed__impl&__31_assoc_type_impl_false_definition
   :skolemid skolem_internal_proj__core!ops.function.FnOnce./Output_alloc__boxed__impl&__31_assoc_type_impl_false_definition
)))
(declare-fun vstd!seq.Seq.len.? (Dcr Type Poly) Int)
(declare-fun vstd!seq.Seq.index.? (Dcr Type Poly Poly) Poly)
(declare-fun vstd!seq.impl&%2.spec_index.? (Dcr Type Poly Poly) Poly)
(declare-fun vstd!seq.Seq.subrange.? (Dcr Type Poly Poly Poly) Poly)
(declare-fun vstd!seq.impl&%2.spec_index_range.? (Dcr Type Dcr Type Dcr Type Poly Poly
  Poly
 ) Poly
)
(declare-fun vstd!seq.Seq.empty.? (Dcr Type) Poly)
(declare-fun vstd!seq.Seq.push.? (Dcr Type Poly Poly) Poly)
(declare-fun vstd!seq.Seq.new.? (Dcr Type Poly Poly) Poly)
(declare-fun vstd!seq.Seq.add.? (Dcr Type Poly Poly) Poly)
(declare-fun vstd!seq_lib.impl&%0.drop_last.? (Dcr Type Poly) Poly)
(declare-fun vstd!seq.Seq.last.? (Dcr Type Poly) Poly)
(declare-fun vstd!seq.impl&%2.spec_add.? (Dcr Type Poly Poly) Poly)
(declare-fun vstd!seq.impl&%2.take.? (Dcr Type Poly Poly) Poly)
(declare-fun vstd!seq.impl&%2.spec_index_range_to.? (Dcr Type Dcr Type Poly Poly)
 Poly
)
(declare-fun vstd!seq.impl&%2.skip.? (Dcr Type Poly Poly) Poly)
(declare-fun vstd!seq_lib.impl&%0.drop_first.? (Dcr Type Poly) Poly)
(declare-fun vstd!arithmetic.power.pow.? (Poly Poly) Int)
(declare-fun vstd!arithmetic.power.rec%pow.? (Poly Poly Fuel) Int)
(declare-fun vstd!endian.Base.base.? (Dcr Type) Poly)
(declare-fun vstd!endian.Base.base%default%.? (Dcr Type) Poly)
(declare-fun vstd!endian.BasePow2.bits.? (Dcr Type) Poly)
(declare-fun vstd!endian.BasePow2.bits%default%.? (Dcr Type) Poly)
(declare-fun vstd!arithmetic.power2.pow2.? (Poly) Int)
(declare-fun vstd!seq.impl&%2.first.? (Dcr Type Poly) Poly)
(declare-fun vstd!seq_lib.impl&%0.map.? (Dcr Type Dcr Type Poly Poly) Poly)
(declare-fun vstd!endian.endianness.? (Poly) vstd!endian.Endian.)
(declare-fun vstd!endian.base_upper_bound_excl.? (Dcr Type Poly) Int)
(declare-fun vstd!endian.impl&%0.in_bounds.? (Dcr Type Poly) Bool)
(declare-fun vstd!endian.impl&%0.wf.? (Dcr Type Poly) Bool)
(declare-fun vstd!endian.impl&%0.new.? (Dcr Type Poly Poly) vstd!endian.EndianNat.)
(declare-fun vstd!endian.impl&%0.new_default.? (Dcr Type Poly) vstd!endian.EndianNat.)
(declare-fun vstd!endian.impl&%0.len.? (Dcr Type Poly) Int)
(declare-fun vstd!endian.impl&%0.index.? (Dcr Type Poly Poly) Int)
(declare-fun vstd!endian.impl&%0.spec_index.? (Dcr Type Poly Poly) Int)
(declare-fun vstd!endian.impl&%0.first.? (Dcr Type Poly) Int)
(declare-fun vstd!endian.impl&%0.last.? (Dcr Type Poly) Int)
(declare-fun vstd!endian.impl&%0.least.? (Dcr Type Poly) Int)
(declare-fun vstd!endian.impl&%0.most.? (Dcr Type Poly) Int)
(declare-fun vstd!endian.impl&%0.skip.? (Dcr Type Poly Poly) vstd!endian.EndianNat.)
(declare-fun vstd!endian.impl&%0.take.? (Dcr Type Poly Poly) vstd!endian.EndianNat.)
(declare-fun vstd!endian.impl&%0.spec_index_range_from.? (Dcr Type Dcr Type Poly Poly)
 vstd!endian.EndianNat.
)
(declare-fun vstd!endian.impl&%0.spec_index_range_to.? (Dcr Type Dcr Type Poly Poly)
 vstd!endian.EndianNat.
)
(declare-fun vstd!endian.impl&%0.skip_least.? (Dcr Type Poly Poly) vstd!endian.EndianNat.)
(declare-fun vstd!endian.impl&%0.skip_most.? (Dcr Type Poly Poly) vstd!endian.EndianNat.)
(declare-fun vstd!endian.impl&%0.take_least.? (Dcr Type Poly Poly) vstd!endian.EndianNat.)
(declare-fun vstd!endian.impl&%0.take_most.? (Dcr Type Poly Poly) vstd!endian.EndianNat.)
(declare-fun vstd!endian.impl&%0.drop_first.? (Dcr Type Poly) vstd!endian.EndianNat.)
(declare-fun vstd!endian.impl&%0.drop_last.? (Dcr Type Poly) vstd!endian.EndianNat.)
(declare-fun vstd!endian.impl&%0.drop_least.? (Dcr Type Poly) vstd!endian.EndianNat.)
(declare-fun vstd!endian.impl&%0.drop_most.? (Dcr Type Poly) vstd!endian.EndianNat.)
(declare-fun vstd!endian.impl&%0.append.? (Dcr Type Poly Poly) vstd!endian.EndianNat.)
(declare-fun vstd!endian.impl&%0.append_least.? (Dcr Type Poly Poly) vstd!endian.EndianNat.)
(declare-fun vstd!endian.impl&%0.append_most.? (Dcr Type Poly Poly) vstd!endian.EndianNat.)
(declare-fun vstd!endian.impl&%0.push_first.? (Dcr Type Poly Poly) vstd!endian.EndianNat.)
(declare-fun vstd!endian.impl&%0.push_last.? (Dcr Type Poly Poly) vstd!endian.EndianNat.)
(declare-fun vstd!endian.impl&%0.push_least.? (Dcr Type Poly Poly) vstd!endian.EndianNat.)
(declare-fun vstd!endian.impl&%0.push_most.? (Dcr Type Poly Poly) vstd!endian.EndianNat.)
(declare-fun vstd!endian.impl&%0.to_nat.? (Dcr Type Poly) Int)
(declare-fun vstd!endian.impl&%0.rec%to_nat.? (Dcr Type Poly Fuel) Int)
(declare-fun vstd!endian.impl&%0.from_nat.? (Dcr Type Poly Poly) vstd!endian.EndianNat.)
(declare-fun vstd!endian.impl&%0.rec%from_nat.? (Dcr Type Poly Poly Fuel) vstd!endian.EndianNat.)
(declare-fun vstd!endian.impl&%0.to_nat_most.? (Dcr Type Poly) Int)
(declare-fun vstd!endian.impl&%0.rec%to_nat_most.? (Dcr Type Poly Fuel) Int)
(declare-fun vstd!endian.impl&%1.exp.? (Dcr Type Dcr Type) Int)
(declare-fun vstd!endian.impl&%1.from_big.? (Dcr Type Dcr Type Poly) vstd!endian.EndianNat.)
(declare-fun vstd!endian.impl&%1.rec%from_big.? (Dcr Type Dcr Type Poly Fuel) vstd!endian.EndianNat.)
(declare-fun vstd!endian.impl&%1.to_big.? (Dcr Type Dcr Type Poly) vstd!endian.EndianNat.)
(declare-fun vstd!endian.impl&%1.rec%to_big.? (Dcr Type Dcr Type Poly Fuel) vstd!endian.EndianNat.)
(declare-fun vstd!endian.to_big_from_digits.? (Dcr Type Dcr Type Poly) vstd!endian.EndianNat.)
(assert
 (tr_bound%core!marker.Tuple. $ TYPE%tuple%0.)
)
(assert
 (tr_bound%core!clone.Clone. $ TYPE%tuple%0.)
)
(assert
 (tr_bound%core!marker.Copy. $ TYPE%tuple%0.)
)
(assert
 (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly)) (!
   (=>
    (has_type spec%param%0! (TYPE%vstd!seq.Seq. A&. A&))
    (<= 0 (vstd!seq.Seq.len.? A&. A& spec%param%0!))
   )
   :pattern ((vstd!seq.Seq.len.? A&. A& spec%param%0!))
   :qid internal_vstd!seq.Seq.len.?_pre_post_definition
   :skolemid skolem_internal_vstd!seq.Seq.len.?_pre_post_definition
)))
(declare-fun req%vstd!seq.Seq.index. (Dcr Type Poly Poly) Bool)
(declare-const %%global_location_label%%0 Bool)
(assert
 (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (= (req%vstd!seq.Seq.index. A&. A& spec%param%0! spec%param%1!) (=>
     %%global_location_label%%0
     (let
      ((tmp%%$ 0))
      (let
       ((tmp%%$1 (%I spec%param%1!)))
       (let
        ((tmp%%$2 (vstd!seq.Seq.len.? A&. A& spec%param%0!)))
        (and
         (<= tmp%%$ tmp%%$1)
         (< tmp%%$1 tmp%%$2)
   ))))))
   :pattern ((req%vstd!seq.Seq.index. A&. A& spec%param%0! spec%param%1!))
   :qid internal_req__vstd!seq.Seq.index._definition
   :skolemid skolem_internal_req__vstd!seq.Seq.index._definition
)))
(assert
 (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (=>
    (and
     (has_type spec%param%0! (TYPE%vstd!seq.Seq. A&. A&))
     (has_type spec%param%1! INT)
    )
    (has_type (vstd!seq.Seq.index.? A&. A& spec%param%0! spec%param%1!) A&)
   )
   :pattern ((vstd!seq.Seq.index.? A&. A& spec%param%0! spec%param%1!))
   :qid internal_vstd!seq.Seq.index.?_pre_post_definition
   :skolemid skolem_internal_vstd!seq.Seq.index.?_pre_post_definition
)))
(declare-fun req%vstd!seq.impl&%2.spec_index. (Dcr Type Poly Poly) Bool)
(declare-const %%global_location_label%%1 Bool)
(assert
 (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (= (req%vstd!seq.impl&%2.spec_index. A&. A& spec%param%0! spec%param%1!) (=>
     %%global_location_label%%1
     (let
      ((tmp%%$ 0))
      (let
       ((tmp%%$1 (%I spec%param%1!)))
       (let
        ((tmp%%$2 (vstd!seq.Seq.len.? A&. A& spec%param%0!)))
        (and
         (<= tmp%%$ tmp%%$1)
         (< tmp%%$1 tmp%%$2)
   ))))))
   :pattern ((req%vstd!seq.impl&%2.spec_index. A&. A& spec%param%0! spec%param%1!))
   :qid internal_req__vstd!seq.impl&__2.spec_index._definition
   :skolemid skolem_internal_req__vstd!seq.impl&__2.spec_index._definition
)))
(assert
 (fuel_bool_default fuel%vstd!seq.impl&%2.spec_index.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!seq.impl&%2.spec_index.)
  (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
    (= (vstd!seq.impl&%2.spec_index.? A&. A& spec%param%0! spec%param%1!) (vstd!seq.Seq.index.?
      A&. A& spec%param%0! spec%param%1!
    ))
    :pattern ((vstd!seq.impl&%2.spec_index.? A&. A& spec%param%0! spec%param%1!))
    :qid internal_vstd!seq.impl&__2.spec_index.?_definition
    :skolemid skolem_internal_vstd!seq.impl&__2.spec_index.?_definition
))))
(assert
 (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (=>
    (and
     (has_type spec%param%0! (TYPE%vstd!seq.Seq. A&. A&))
     (has_type spec%param%1! INT)
    )
    (has_type (vstd!seq.impl&%2.spec_index.? A&. A& spec%param%0! spec%param%1!) A&)
   )
   :pattern ((vstd!seq.impl&%2.spec_index.? A&. A& spec%param%0! spec%param%1!))
   :qid internal_vstd!seq.impl&__2.spec_index.?_pre_post_definition
   :skolemid skolem_internal_vstd!seq.impl&__2.spec_index.?_pre_post_definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!seq.lemma_seq_index_decreases.)
  (forall ((A&. Dcr) (A& Type) (s! Poly) (i! Poly)) (!
    (=>
     (and
      (has_type s! (TYPE%vstd!seq.Seq. A&. A&))
      (has_type i! INT)
     )
     (=>
      (and
       (sized A&.)
       (let
        ((tmp%%$ 0))
        (let
         ((tmp%%$1 (%I i!)))
         (let
          ((tmp%%$2 (vstd!seq.Seq.len.? A&. A& s!)))
          (and
           (<= tmp%%$ tmp%%$1)
           (< tmp%%$1 tmp%%$2)
      )))))
      (height_lt (height (vstd!seq.Seq.index.? A&. A& s! i!)) (height s!))
    ))
    :pattern ((height (vstd!seq.Seq.index.? A&. A& s! i!)))
    :qid user_vstd__seq__lemma_seq_index_decreases_0
    :skolemid skolem_user_vstd__seq__lemma_seq_index_decreases_0
))))
(assert
 (tr_bound%core!clone.Clone. $ INT)
)
(assert
 (tr_bound%core!marker.Copy. $ INT)
)
(assert
 (tr_bound%verus_builtin!Integer. $ INT)
)
(declare-fun req%vstd!seq.Seq.subrange. (Dcr Type Poly Poly Poly) Bool)
(declare-const %%global_location_label%%2 Bool)
(assert
 (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly) (spec%param%1! Poly) (spec%param%2!
    Poly
   )
  ) (!
   (= (req%vstd!seq.Seq.subrange. A&. A& spec%param%0! spec%param%1! spec%param%2!) (
     =>
     %%global_location_label%%2
     (let
      ((tmp%%$ 0))
      (let
       ((tmp%%$1 (%I spec%param%1!)))
       (let
        ((tmp%%$2 (%I spec%param%2!)))
        (let
         ((tmp%%$3 (vstd!seq.Seq.len.? A&. A& spec%param%0!)))
         (and
          (and
           (<= tmp%%$ tmp%%$1)
           (<= tmp%%$1 tmp%%$2)
          )
          (<= tmp%%$2 tmp%%$3)
   )))))))
   :pattern ((req%vstd!seq.Seq.subrange. A&. A& spec%param%0! spec%param%1! spec%param%2!))
   :qid internal_req__vstd!seq.Seq.subrange._definition
   :skolemid skolem_internal_req__vstd!seq.Seq.subrange._definition
)))
(assert
 (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly) (spec%param%1! Poly) (spec%param%2!
    Poly
   )
  ) (!
   (=>
    (and
     (has_type spec%param%0! (TYPE%vstd!seq.Seq. A&. A&))
     (has_type spec%param%1! INT)
     (has_type spec%param%2! INT)
    )
    (has_type (vstd!seq.Seq.subrange.? A&. A& spec%param%0! spec%param%1! spec%param%2!)
     (TYPE%vstd!seq.Seq. A&. A&)
   ))
   :pattern ((vstd!seq.Seq.subrange.? A&. A& spec%param%0! spec%param%1! spec%param%2!))
   :qid internal_vstd!seq.Seq.subrange.?_pre_post_definition
   :skolemid skolem_internal_vstd!seq.Seq.subrange.?_pre_post_definition
)))
(declare-fun req%vstd!seq.impl&%2.spec_index_range. (Dcr Type Dcr Type Dcr Type Poly
  Poly Poly
 ) Bool
)
(declare-const %%global_location_label%%3 Bool)
(assert
 (forall ((A&. Dcr) (A& Type) (I&. Dcr) (I& Type) (J&. Dcr) (J& Type) (spec%param%0!
    Poly
   ) (spec%param%1! Poly) (spec%param%2! Poly)
  ) (!
   (= (req%vstd!seq.impl&%2.spec_index_range. A&. A& I&. I& J&. J& spec%param%0! spec%param%1!
     spec%param%2!
    ) (=>
     %%global_location_label%%3
     (let
      ((tmp%%$ 0))
      (let
       ((tmp%%$1 (%I spec%param%1!)))
       (let
        ((tmp%%$2 (%I spec%param%2!)))
        (let
         ((tmp%%$3 (vstd!seq.Seq.len.? A&. A& spec%param%0!)))
         (and
          (and
           (<= tmp%%$ tmp%%$1)
           (<= tmp%%$1 tmp%%$2)
          )
          (<= tmp%%$2 tmp%%$3)
   )))))))
   :pattern ((req%vstd!seq.impl&%2.spec_index_range. A&. A& I&. I& J&. J& spec%param%0!
     spec%param%1! spec%param%2!
   ))
   :qid internal_req__vstd!seq.impl&__2.spec_index_range._definition
   :skolemid skolem_internal_req__vstd!seq.impl&__2.spec_index_range._definition
)))
(assert
 (fuel_bool_default fuel%vstd!seq.impl&%2.spec_index_range.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!seq.impl&%2.spec_index_range.)
  (forall ((A&. Dcr) (A& Type) (I&. Dcr) (I& Type) (J&. Dcr) (J& Type) (spec%param%0!
     Poly
    ) (spec%param%1! Poly) (spec%param%2! Poly)
   ) (!
    (= (vstd!seq.impl&%2.spec_index_range.? A&. A& I&. I& J&. J& spec%param%0! spec%param%1!
      spec%param%2!
     ) (vstd!seq.Seq.subrange.? A&. A& spec%param%0! spec%param%1! spec%param%2!)
    )
    :pattern ((vstd!seq.impl&%2.spec_index_range.? A&. A& I&. I& J&. J& spec%param%0! spec%param%1!
      spec%param%2!
    ))
    :qid internal_vstd!seq.impl&__2.spec_index_range.?_definition
    :skolemid skolem_internal_vstd!seq.impl&__2.spec_index_range.?_definition
))))
(assert
 (forall ((A&. Dcr) (A& Type) (I&. Dcr) (I& Type) (J&. Dcr) (J& Type) (spec%param%0!
    Poly
   ) (spec%param%1! Poly) (spec%param%2! Poly)
  ) (!
   (=>
    (and
     (has_type spec%param%0! (TYPE%vstd!seq.Seq. A&. A&))
     (has_type spec%param%1! I&)
     (has_type spec%param%2! J&)
    )
    (has_type (vstd!seq.impl&%2.spec_index_range.? A&. A& I&. I& J&. J& spec%param%0! spec%param%1!
      spec%param%2!
     ) (TYPE%vstd!seq.Seq. A&. A&)
   ))
   :pattern ((vstd!seq.impl&%2.spec_index_range.? A&. A& I&. I& J&. J& spec%param%0! spec%param%1!
     spec%param%2!
   ))
   :qid internal_vstd!seq.impl&__2.spec_index_range.?_pre_post_definition
   :skolemid skolem_internal_vstd!seq.impl&__2.spec_index_range.?_pre_post_definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!seq.lemma_seq_subrange_len.)
  (forall ((A&. Dcr) (A& Type) (s! Poly) (j! Poly) (k! Poly)) (!
    (=>
     (and
      (has_type s! (TYPE%vstd!seq.Seq. A&. A&))
      (has_type j! INT)
      (has_type k! INT)
     )
     (=>
      (and
       (sized A&.)
       (let
        ((tmp%%$ 0))
        (let
         ((tmp%%$1 (%I j!)))
         (let
          ((tmp%%$2 (%I k!)))
          (let
           ((tmp%%$3 (vstd!seq.Seq.len.? A&. A& s!)))
           (and
            (and
             (<= tmp%%$ tmp%%$1)
             (<= tmp%%$1 tmp%%$2)
            )
            (<= tmp%%$2 tmp%%$3)
      ))))))
      (= (vstd!seq.Seq.len.? A&. A& (vstd!seq.Seq.subrange.? A&. A& s! j! k!)) (Sub (%I k!)
        (%I j!)
    ))))
    :pattern ((vstd!seq.Seq.len.? A&. A& (vstd!seq.Seq.subrange.? A&. A& s! j! k!)))
    :qid user_vstd__seq__lemma_seq_subrange_len_0
    :skolemid skolem_user_vstd__seq__lemma_seq_subrange_len_0
))))
(assert
 (=>
  (fuel_bool fuel%vstd!seq.lemma_seq_subrange_index.)
  (forall ((A&. Dcr) (A& Type) (s! Poly) (j! Poly) (k! Poly) (i! Poly)) (!
    (=>
     (and
      (has_type s! (TYPE%vstd!seq.Seq. A&. A&))
      (has_type j! INT)
      (has_type k! INT)
      (has_type i! INT)
     )
     (=>
      (and
       (and
        (sized A&.)
        (let
         ((tmp%%$ 0))
         (let
          ((tmp%%$1 (%I j!)))
          (let
           ((tmp%%$2 (%I k!)))
           (let
            ((tmp%%$3 (vstd!seq.Seq.len.? A&. A& s!)))
            (and
             (and
              (<= tmp%%$ tmp%%$1)
              (<= tmp%%$1 tmp%%$2)
             )
             (<= tmp%%$2 tmp%%$3)
       ))))))
       (let
        ((tmp%%$ 0))
        (let
         ((tmp%%$5 (%I i!)))
         (let
          ((tmp%%$6 (Sub (%I k!) (%I j!))))
          (and
           (<= tmp%%$ tmp%%$5)
           (< tmp%%$5 tmp%%$6)
      )))))
      (= (vstd!seq.Seq.index.? A&. A& (vstd!seq.Seq.subrange.? A&. A& s! j! k!) i!) (vstd!seq.Seq.index.?
        A&. A& s! (I (Add (%I i!) (%I j!)))
    ))))
    :pattern ((vstd!seq.Seq.index.? A&. A& (vstd!seq.Seq.subrange.? A&. A& s! j! k!) i!))
    :qid user_vstd__seq__lemma_seq_subrange_index_0
    :skolemid skolem_user_vstd__seq__lemma_seq_subrange_index_0
))))
(assert
 (=>
  (fuel_bool fuel%vstd!seq.lemma_seq_subrange_decreases.)
  (forall ((A&. Dcr) (A& Type) (s! Poly) (i! Poly) (j! Poly)) (!
    (=>
     (and
      (has_type s! (TYPE%vstd!seq.Seq. A&. A&))
      (has_type i! INT)
      (has_type j! INT)
     )
     (=>
      (and
       (and
        (sized A&.)
        (let
         ((tmp%%$ 0))
         (let
          ((tmp%%$1 (%I i!)))
          (let
           ((tmp%%$2 (%I j!)))
           (let
            ((tmp%%$3 (vstd!seq.Seq.len.? A&. A& s!)))
            (and
             (and
              (<= tmp%%$ tmp%%$1)
              (<= tmp%%$1 tmp%%$2)
             )
             (<= tmp%%$2 tmp%%$3)
       ))))))
       (< (vstd!seq.Seq.len.? A&. A& (vstd!seq.Seq.subrange.? A&. A& s! i! j!)) (vstd!seq.Seq.len.?
         A&. A& s!
      )))
      (height_lt (height (vstd!seq.Seq.subrange.? A&. A& s! i! j!)) (height s!))
    ))
    :pattern ((height (vstd!seq.Seq.subrange.? A&. A& s! i! j!)))
    :qid user_vstd__seq__lemma_seq_subrange_decreases_0
    :skolemid skolem_user_vstd__seq__lemma_seq_subrange_decreases_0
))))
(assert
 (forall ((A&. Dcr) (A& Type)) (!
   (has_type (vstd!seq.Seq.empty.? A&. A&) (TYPE%vstd!seq.Seq. A&. A&))
   :pattern ((vstd!seq.Seq.empty.? A&. A&))
   :qid internal_vstd!seq.Seq.empty.?_pre_post_definition
   :skolemid skolem_internal_vstd!seq.Seq.empty.?_pre_post_definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!seq.lemma_seq_empty.)
  (forall ((A&. Dcr) (A& Type)) (!
    (=>
     (sized A&.)
     (= (vstd!seq.Seq.len.? A&. A& (vstd!seq.Seq.empty.? A&. A&)) 0)
    )
    :pattern ((vstd!seq.Seq.len.? A&. A& (vstd!seq.Seq.empty.? A&. A&)))
    :qid user_vstd__seq__lemma_seq_empty_0
    :skolemid skolem_user_vstd__seq__lemma_seq_empty_0
))))
(assert
 (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (=>
    (and
     (has_type spec%param%0! (TYPE%vstd!seq.Seq. A&. A&))
     (has_type spec%param%1! A&)
    )
    (has_type (vstd!seq.Seq.push.? A&. A& spec%param%0! spec%param%1!) (TYPE%vstd!seq.Seq.
      A&. A&
   )))
   :pattern ((vstd!seq.Seq.push.? A&. A& spec%param%0! spec%param%1!))
   :qid internal_vstd!seq.Seq.push.?_pre_post_definition
   :skolemid skolem_internal_vstd!seq.Seq.push.?_pre_post_definition
)))
(assert
 (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (=>
    (and
     (has_type spec%param%0! NAT)
     (has_type spec%param%1! (TYPE%fun%1. $ INT A&. A&))
    )
    (has_type (vstd!seq.Seq.new.? A&. A& spec%param%0! spec%param%1!) (TYPE%vstd!seq.Seq.
      A&. A&
   )))
   :pattern ((vstd!seq.Seq.new.? A&. A& spec%param%0! spec%param%1!))
   :qid internal_vstd!seq.Seq.new.?_pre_post_definition
   :skolemid skolem_internal_vstd!seq.Seq.new.?_pre_post_definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!seq.lemma_seq_push_len.)
  (forall ((A&. Dcr) (A& Type) (s! Poly) (a! Poly)) (!
    (=>
     (and
      (has_type s! (TYPE%vstd!seq.Seq. A&. A&))
      (has_type a! A&)
     )
     (=>
      (sized A&.)
      (= (vstd!seq.Seq.len.? A&. A& (vstd!seq.Seq.push.? A&. A& s! a!)) (nClip (Add (vstd!seq.Seq.len.?
          A&. A& s!
         ) 1
    )))))
    :pattern ((vstd!seq.Seq.len.? A&. A& (vstd!seq.Seq.push.? A&. A& s! a!)))
    :qid user_vstd__seq__lemma_seq_push_len_0
    :skolemid skolem_user_vstd__seq__lemma_seq_push_len_0
))))
(assert
 (=>
  (fuel_bool fuel%vstd!seq.lemma_seq_new_len.)
  (forall ((A&. Dcr) (A& Type) (len! Poly) (f! Poly)) (!
    (=>
     (and
      (has_type len! NAT)
      (has_type f! (TYPE%fun%1. $ INT A&. A&))
     )
     (=>
      (sized A&.)
      (= (vstd!seq.Seq.len.? A&. A& (vstd!seq.Seq.new.? A&. A& len! f!)) (%I len!))
    ))
    :pattern ((vstd!seq.Seq.len.? A&. A& (vstd!seq.Seq.new.? A&. A& len! f!)))
    :qid user_vstd__seq__lemma_seq_new_len_0
    :skolemid skolem_user_vstd__seq__lemma_seq_new_len_0
))))
(assert
 (=>
  (fuel_bool fuel%vstd!seq.lemma_seq_push_index_same.)
  (forall ((A&. Dcr) (A& Type) (s! Poly) (a! Poly) (i! Poly)) (!
    (=>
     (and
      (has_type s! (TYPE%vstd!seq.Seq. A&. A&))
      (has_type a! A&)
      (has_type i! INT)
     )
     (=>
      (and
       (sized A&.)
       (= (%I i!) (vstd!seq.Seq.len.? A&. A& s!))
      )
      (= (vstd!seq.Seq.index.? A&. A& (vstd!seq.Seq.push.? A&. A& s! a!) i!) a!)
    ))
    :pattern ((vstd!seq.Seq.index.? A&. A& (vstd!seq.Seq.push.? A&. A& s! a!) i!))
    :qid user_vstd__seq__lemma_seq_push_index_same_0
    :skolemid skolem_user_vstd__seq__lemma_seq_push_index_same_0
))))
(assert
 (=>
  (fuel_bool fuel%vstd!seq.lemma_seq_push_index_different.)
  (forall ((A&. Dcr) (A& Type) (s! Poly) (a! Poly) (i! Poly)) (!
    (=>
     (and
      (has_type s! (TYPE%vstd!seq.Seq. A&. A&))
      (has_type a! A&)
      (has_type i! INT)
     )
     (=>
      (and
       (sized A&.)
       (< (%I i!) (vstd!seq.Seq.len.? A&. A& s!))
      )
      (= (vstd!seq.Seq.index.? A&. A& (vstd!seq.Seq.push.? A&. A& s! a!) i!) (vstd!seq.Seq.index.?
        A&. A& s! i!
    ))))
    :pattern ((vstd!seq.Seq.index.? A&. A& (vstd!seq.Seq.push.? A&. A& s! a!) i!))
    :qid user_vstd__seq__lemma_seq_push_index_different_0
    :skolemid skolem_user_vstd__seq__lemma_seq_push_index_different_0
))))
(assert
 (=>
  (fuel_bool fuel%vstd!seq.lemma_seq_new_index.)
  (forall ((A&. Dcr) (A& Type) (len! Poly) (f! Poly) (i! Poly)) (!
    (=>
     (and
      (has_type len! NAT)
      (has_type f! (TYPE%fun%1. $ INT A&. A&))
      (has_type i! INT)
     )
     (=>
      (and
       (sized A&.)
       (let
        ((tmp%%$ 0))
        (let
         ((tmp%%$1 (%I i!)))
         (let
          ((tmp%%$2 (%I len!)))
          (and
           (<= tmp%%$ tmp%%$1)
           (< tmp%%$1 tmp%%$2)
      )))))
      (= (vstd!seq.Seq.index.? A&. A& (vstd!seq.Seq.new.? A&. A& len! f!) i!) (%%apply%%0
        (%Poly%fun%1. f!) i!
    ))))
    :pattern ((vstd!seq.Seq.index.? A&. A& (vstd!seq.Seq.new.? A&. A& len! f!) i!))
    :qid user_vstd__seq__lemma_seq_new_index_0
    :skolemid skolem_user_vstd__seq__lemma_seq_new_index_0
))))
(assert
 (=>
  (fuel_bool fuel%vstd!seq.lemma_seq_ext_equal.)
  (forall ((A&. Dcr) (A& Type) (s1! Poly) (s2! Poly)) (!
    (=>
     (and
      (has_type s1! (TYPE%vstd!seq.Seq. A&. A&))
      (has_type s2! (TYPE%vstd!seq.Seq. A&. A&))
     )
     (=>
      (sized A&.)
      (= (ext_eq false (TYPE%vstd!seq.Seq. A&. A&) s1! s2!) (and
        (= (vstd!seq.Seq.len.? A&. A& s1!) (vstd!seq.Seq.len.? A&. A& s2!))
        (forall ((i$ Poly)) (!
          (=>
           (has_type i$ INT)
           (=>
            (let
             ((tmp%%$ 0))
             (let
              ((tmp%%$1 (%I i$)))
              (let
               ((tmp%%$2 (vstd!seq.Seq.len.? A&. A& s1!)))
               (and
                (<= tmp%%$ tmp%%$1)
                (< tmp%%$1 tmp%%$2)
            ))))
            (= (vstd!seq.Seq.index.? A&. A& s1! i$) (vstd!seq.Seq.index.? A&. A& s2! i$))
          ))
          :pattern ((vstd!seq.Seq.index.? A&. A& s1! i$))
          :pattern ((vstd!seq.Seq.index.? A&. A& s2! i$))
          :qid user_vstd__seq__lemma_seq_ext_equal_0
          :skolemid skolem_user_vstd__seq__lemma_seq_ext_equal_0
    ))))))
    :pattern ((ext_eq false (TYPE%vstd!seq.Seq. A&. A&) s1! s2!))
    :qid user_vstd__seq__lemma_seq_ext_equal_1
    :skolemid skolem_user_vstd__seq__lemma_seq_ext_equal_1
))))
(assert
 (=>
  (fuel_bool fuel%vstd!seq.lemma_seq_ext_equal_deep.)
  (forall ((A&. Dcr) (A& Type) (s1! Poly) (s2! Poly)) (!
    (=>
     (and
      (has_type s1! (TYPE%vstd!seq.Seq. A&. A&))
      (has_type s2! (TYPE%vstd!seq.Seq. A&. A&))
     )
     (=>
      (sized A&.)
      (= (ext_eq true (TYPE%vstd!seq.Seq. A&. A&) s1! s2!) (and
        (= (vstd!seq.Seq.len.? A&. A& s1!) (vstd!seq.Seq.len.? A&. A& s2!))
        (forall ((i$ Poly)) (!
          (=>
           (has_type i$ INT)
           (=>
            (let
             ((tmp%%$ 0))
             (let
              ((tmp%%$1 (%I i$)))
              (let
               ((tmp%%$2 (vstd!seq.Seq.len.? A&. A& s1!)))
               (and
                (<= tmp%%$ tmp%%$1)
                (< tmp%%$1 tmp%%$2)
            ))))
            (ext_eq true A& (vstd!seq.Seq.index.? A&. A& s1! i$) (vstd!seq.Seq.index.? A&. A& s2!
              i$
          ))))
          :pattern ((vstd!seq.Seq.index.? A&. A& s1! i$))
          :pattern ((vstd!seq.Seq.index.? A&. A& s2! i$))
          :qid user_vstd__seq__lemma_seq_ext_equal_deep_0
          :skolemid skolem_user_vstd__seq__lemma_seq_ext_equal_deep_0
    ))))))
    :pattern ((ext_eq true (TYPE%vstd!seq.Seq. A&. A&) s1! s2!))
    :qid user_vstd__seq__lemma_seq_ext_equal_deep_1
    :skolemid skolem_user_vstd__seq__lemma_seq_ext_equal_deep_1
))))
(assert
 (=>
  (fuel_bool fuel%vstd!seq.lemma_seq_two_subranges_index.)
  (forall ((A&. Dcr) (A& Type) (s! Poly) (j! Poly) (k1! Poly) (k2! Poly) (i! Poly))
   (!
    (=>
     (and
      (has_type s! (TYPE%vstd!seq.Seq. A&. A&))
      (has_type j! INT)
      (has_type k1! INT)
      (has_type k2! INT)
      (has_type i! INT)
     )
     (=>
      (and
       (and
        (and
         (and
          (sized A&.)
          (let
           ((tmp%%$ 0))
           (let
            ((tmp%%$1 (%I j!)))
            (let
             ((tmp%%$2 (%I k1!)))
             (let
              ((tmp%%$3 (vstd!seq.Seq.len.? A&. A& s!)))
              (and
               (and
                (<= tmp%%$ tmp%%$1)
                (<= tmp%%$1 tmp%%$2)
               )
               (<= tmp%%$2 tmp%%$3)
         ))))))
         (let
          ((tmp%%$ 0))
          (let
           ((tmp%%$5 (%I j!)))
           (let
            ((tmp%%$6 (%I k2!)))
            (let
             ((tmp%%$7 (vstd!seq.Seq.len.? A&. A& s!)))
             (and
              (and
               (<= tmp%%$ tmp%%$5)
               (<= tmp%%$5 tmp%%$6)
              )
              (<= tmp%%$6 tmp%%$7)
        ))))))
        (let
         ((tmp%%$ 0))
         (let
          ((tmp%%$9 (%I i!)))
          (let
           ((tmp%%$10 (Sub (%I k1!) (%I j!))))
           (and
            (<= tmp%%$ tmp%%$9)
            (< tmp%%$9 tmp%%$10)
       )))))
       (let
        ((tmp%%$ 0))
        (let
         ((tmp%%$12 (%I i!)))
         (let
          ((tmp%%$13 (Sub (%I k2!) (%I j!))))
          (and
           (<= tmp%%$ tmp%%$12)
           (< tmp%%$12 tmp%%$13)
      )))))
      (= (vstd!seq.Seq.index.? A&. A& (vstd!seq.Seq.subrange.? A&. A& s! j! k1!) i!) (vstd!seq.Seq.index.?
        A&. A& (vstd!seq.Seq.subrange.? A&. A& s! j! k2!) i!
    ))))
    :pattern ((vstd!seq.Seq.index.? A&. A& (vstd!seq.Seq.subrange.? A&. A& s! j! k1!) i!)
     (vstd!seq.Seq.subrange.? A&. A& s! j! k2!)
    )
    :qid user_vstd__seq__lemma_seq_two_subranges_index_0
    :skolemid skolem_user_vstd__seq__lemma_seq_two_subranges_index_0
))))
(assert
 (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (=>
    (and
     (has_type spec%param%0! (TYPE%vstd!seq.Seq. A&. A&))
     (has_type spec%param%1! (TYPE%vstd!seq.Seq. A&. A&))
    )
    (has_type (vstd!seq.Seq.add.? A&. A& spec%param%0! spec%param%1!) (TYPE%vstd!seq.Seq.
      A&. A&
   )))
   :pattern ((vstd!seq.Seq.add.? A&. A& spec%param%0! spec%param%1!))
   :qid internal_vstd!seq.Seq.add.?_pre_post_definition
   :skolemid skolem_internal_vstd!seq.Seq.add.?_pre_post_definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!seq.lemma_seq_add_len.)
  (forall ((A&. Dcr) (A& Type) (s1! Poly) (s2! Poly)) (!
    (=>
     (and
      (has_type s1! (TYPE%vstd!seq.Seq. A&. A&))
      (has_type s2! (TYPE%vstd!seq.Seq. A&. A&))
     )
     (=>
      (sized A&.)
      (= (vstd!seq.Seq.len.? A&. A& (vstd!seq.Seq.add.? A&. A& s1! s2!)) (nClip (Add (vstd!seq.Seq.len.?
          A&. A& s1!
         ) (vstd!seq.Seq.len.? A&. A& s2!)
    )))))
    :pattern ((vstd!seq.Seq.len.? A&. A& (vstd!seq.Seq.add.? A&. A& s1! s2!)))
    :qid user_vstd__seq__lemma_seq_add_len_0
    :skolemid skolem_user_vstd__seq__lemma_seq_add_len_0
))))
(assert
 (=>
  (fuel_bool fuel%vstd!seq.lemma_seq_add_index1.)
  (forall ((A&. Dcr) (A& Type) (s1! Poly) (s2! Poly) (i! Poly)) (!
    (=>
     (and
      (has_type s1! (TYPE%vstd!seq.Seq. A&. A&))
      (has_type s2! (TYPE%vstd!seq.Seq. A&. A&))
      (has_type i! INT)
     )
     (=>
      (and
       (sized A&.)
       (< (%I i!) (vstd!seq.Seq.len.? A&. A& s1!))
      )
      (= (vstd!seq.Seq.index.? A&. A& (vstd!seq.Seq.add.? A&. A& s1! s2!) i!) (vstd!seq.Seq.index.?
        A&. A& s1! i!
    ))))
    :pattern ((vstd!seq.Seq.index.? A&. A& (vstd!seq.Seq.add.? A&. A& s1! s2!) i!))
    :qid user_vstd__seq__lemma_seq_add_index1_0
    :skolemid skolem_user_vstd__seq__lemma_seq_add_index1_0
))))
(assert
 (=>
  (fuel_bool fuel%vstd!seq.lemma_seq_add_index2.)
  (forall ((A&. Dcr) (A& Type) (s1! Poly) (s2! Poly) (i! Poly)) (!
    (=>
     (and
      (has_type s1! (TYPE%vstd!seq.Seq. A&. A&))
      (has_type s2! (TYPE%vstd!seq.Seq. A&. A&))
      (has_type i! INT)
     )
     (=>
      (and
       (sized A&.)
       (let
        ((tmp%%$ (vstd!seq.Seq.len.? A&. A& s1!)))
        (let
         ((tmp%%$1 (%I i!)))
         (let
          ((tmp%%$2 (nClip (Add (vstd!seq.Seq.len.? A&. A& s1!) (vstd!seq.Seq.len.? A&. A& s2!)))))
          (and
           (<= tmp%%$ tmp%%$1)
           (< tmp%%$1 tmp%%$2)
      )))))
      (= (vstd!seq.Seq.index.? A&. A& (vstd!seq.Seq.add.? A&. A& s1! s2!) i!) (vstd!seq.Seq.index.?
        A&. A& s2! (I (Sub (%I i!) (vstd!seq.Seq.len.? A&. A& s1!)))
    ))))
    :pattern ((vstd!seq.Seq.index.? A&. A& (vstd!seq.Seq.add.? A&. A& s1! s2!) i!))
    :qid user_vstd__seq__lemma_seq_add_index2_0
    :skolemid skolem_user_vstd__seq__lemma_seq_add_index2_0
))))
(declare-fun req%vstd!seq_lib.impl&%0.drop_last. (Dcr Type Poly) Bool)
(declare-const %%global_location_label%%4 Bool)
(assert
 (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly)) (!
   (= (req%vstd!seq_lib.impl&%0.drop_last. A&. A& spec%param%0!) (=>
     %%global_location_label%%4
     (>= (vstd!seq.Seq.len.? A&. A& spec%param%0!) 1)
   ))
   :pattern ((req%vstd!seq_lib.impl&%0.drop_last. A&. A& spec%param%0!))
   :qid internal_req__vstd!seq_lib.impl&__0.drop_last._definition
   :skolemid skolem_internal_req__vstd!seq_lib.impl&__0.drop_last._definition
)))
(assert
 (fuel_bool_default fuel%vstd!seq_lib.impl&%0.drop_last.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!seq_lib.impl&%0.drop_last.)
  (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly)) (!
    (= (vstd!seq_lib.impl&%0.drop_last.? A&. A& spec%param%0!) (vstd!seq.Seq.subrange.?
      A&. A& spec%param%0! (I 0) (I (Sub (vstd!seq.Seq.len.? A&. A& spec%param%0!) 1))
    ))
    :pattern ((vstd!seq_lib.impl&%0.drop_last.? A&. A& spec%param%0!))
    :qid internal_vstd!seq_lib.impl&__0.drop_last.?_definition
    :skolemid skolem_internal_vstd!seq_lib.impl&__0.drop_last.?_definition
))))
(assert
 (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly)) (!
   (=>
    (has_type spec%param%0! (TYPE%vstd!seq.Seq. A&. A&))
    (has_type (vstd!seq_lib.impl&%0.drop_last.? A&. A& spec%param%0!) (TYPE%vstd!seq.Seq.
      A&. A&
   )))
   :pattern ((vstd!seq_lib.impl&%0.drop_last.? A&. A& spec%param%0!))
   :qid internal_vstd!seq_lib.impl&__0.drop_last.?_pre_post_definition
   :skolemid skolem_internal_vstd!seq_lib.impl&__0.drop_last.?_pre_post_definition
)))
(declare-fun req%vstd!seq.Seq.last. (Dcr Type Poly) Bool)
(declare-const %%global_location_label%%5 Bool)
(assert
 (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly)) (!
   (= (req%vstd!seq.Seq.last. A&. A& spec%param%0!) (=>
     %%global_location_label%%5
     (< 0 (vstd!seq.Seq.len.? A&. A& spec%param%0!))
   ))
   :pattern ((req%vstd!seq.Seq.last. A&. A& spec%param%0!))
   :qid internal_req__vstd!seq.Seq.last._definition
   :skolemid skolem_internal_req__vstd!seq.Seq.last._definition
)))
(assert
 (fuel_bool_default fuel%vstd!seq.Seq.last.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!seq.Seq.last.)
  (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly)) (!
    (= (vstd!seq.Seq.last.? A&. A& spec%param%0!) (vstd!seq.Seq.index.? A&. A& spec%param%0!
      (I (Sub (vstd!seq.Seq.len.? A&. A& spec%param%0!) 1))
    ))
    :pattern ((vstd!seq.Seq.last.? A&. A& spec%param%0!))
    :qid internal_vstd!seq.Seq.last.?_definition
    :skolemid skolem_internal_vstd!seq.Seq.last.?_definition
))))
(assert
 (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly)) (!
   (=>
    (has_type spec%param%0! (TYPE%vstd!seq.Seq. A&. A&))
    (has_type (vstd!seq.Seq.last.? A&. A& spec%param%0!) A&)
   )
   :pattern ((vstd!seq.Seq.last.? A&. A& spec%param%0!))
   :qid internal_vstd!seq.Seq.last.?_pre_post_definition
   :skolemid skolem_internal_vstd!seq.Seq.last.?_pre_post_definition
)))
(assert
 (fuel_bool_default fuel%vstd!seq.impl&%2.spec_add.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!seq.impl&%2.spec_add.)
  (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
    (= (vstd!seq.impl&%2.spec_add.? A&. A& spec%param%0! spec%param%1!) (vstd!seq.Seq.add.?
      A&. A& spec%param%0! spec%param%1!
    ))
    :pattern ((vstd!seq.impl&%2.spec_add.? A&. A& spec%param%0! spec%param%1!))
    :qid internal_vstd!seq.impl&__2.spec_add.?_definition
    :skolemid skolem_internal_vstd!seq.impl&__2.spec_add.?_definition
))))
(assert
 (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (=>
    (and
     (has_type spec%param%0! (TYPE%vstd!seq.Seq. A&. A&))
     (has_type spec%param%1! (TYPE%vstd!seq.Seq. A&. A&))
    )
    (has_type (vstd!seq.impl&%2.spec_add.? A&. A& spec%param%0! spec%param%1!) (TYPE%vstd!seq.Seq.
      A&. A&
   )))
   :pattern ((vstd!seq.impl&%2.spec_add.? A&. A& spec%param%0! spec%param%1!))
   :qid internal_vstd!seq.impl&__2.spec_add.?_pre_post_definition
   :skolemid skolem_internal_vstd!seq.impl&__2.spec_add.?_pre_post_definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!seq_lib.impl&%0.add_empty_left.)
  (forall ((A&. Dcr) (A& Type) (a! Poly) (b! Poly)) (!
    (=>
     (and
      (has_type a! (TYPE%vstd!seq.Seq. A&. A&))
      (has_type b! (TYPE%vstd!seq.Seq. A&. A&))
     )
     (=>
      (and
       (sized A&.)
       (= (vstd!seq.Seq.len.? A&. A& a!) 0)
      )
      (= (vstd!seq.Seq.add.? A&. A& a! b!) b!)
    ))
    :pattern ((vstd!seq.Seq.add.? A&. A& a! b!))
    :qid user_vstd__seq__Seq__add_empty_left_0
    :skolemid skolem_user_vstd__seq__Seq__add_empty_left_0
))))
(assert
 (=>
  (fuel_bool fuel%vstd!seq_lib.impl&%0.add_empty_right.)
  (forall ((A&. Dcr) (A& Type) (a! Poly) (b! Poly)) (!
    (=>
     (and
      (has_type a! (TYPE%vstd!seq.Seq. A&. A&))
      (has_type b! (TYPE%vstd!seq.Seq. A&. A&))
     )
     (=>
      (and
       (sized A&.)
       (= (vstd!seq.Seq.len.? A&. A& b!) 0)
      )
      (= (vstd!seq.Seq.add.? A&. A& a! b!) a!)
    ))
    :pattern ((vstd!seq.Seq.add.? A&. A& a! b!))
    :qid user_vstd__seq__Seq__add_empty_right_0
    :skolemid skolem_user_vstd__seq__Seq__add_empty_right_0
))))
(assert
 (=>
  (fuel_bool fuel%vstd!seq_lib.impl&%0.push_distributes_over_add.)
  (forall ((A&. Dcr) (A& Type) (a! Poly) (b! Poly) (elt! Poly)) (!
    (=>
     (and
      (has_type a! (TYPE%vstd!seq.Seq. A&. A&))
      (has_type b! (TYPE%vstd!seq.Seq. A&. A&))
      (has_type elt! A&)
     )
     (=>
      (sized A&.)
      (= (vstd!seq.Seq.push.? A&. A& (vstd!seq.Seq.add.? A&. A& a! b!) elt!) (vstd!seq.Seq.add.?
        A&. A& a! (vstd!seq.Seq.push.? A&. A& b! elt!)
    ))))
    :pattern ((vstd!seq.Seq.push.? A&. A& (vstd!seq.Seq.add.? A&. A& a! b!) elt!))
    :qid user_vstd__seq__Seq__push_distributes_over_add_0
    :skolemid skolem_user_vstd__seq__Seq__push_distributes_over_add_0
))))
(assert
 (fuel_bool_default fuel%vstd!seq.impl&%2.take.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!seq.impl&%2.take.)
  (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
    (= (vstd!seq.impl&%2.take.? A&. A& spec%param%0! spec%param%1!) (vstd!seq.Seq.subrange.?
      A&. A& spec%param%0! (I 0) spec%param%1!
    ))
    :pattern ((vstd!seq.impl&%2.take.? A&. A& spec%param%0! spec%param%1!))
    :qid internal_vstd!seq.impl&__2.take.?_definition
    :skolemid skolem_internal_vstd!seq.impl&__2.take.?_definition
))))
(assert
 (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (=>
    (and
     (has_type spec%param%0! (TYPE%vstd!seq.Seq. A&. A&))
     (has_type spec%param%1! INT)
    )
    (has_type (vstd!seq.impl&%2.take.? A&. A& spec%param%0! spec%param%1!) (TYPE%vstd!seq.Seq.
      A&. A&
   )))
   :pattern ((vstd!seq.impl&%2.take.? A&. A& spec%param%0! spec%param%1!))
   :qid internal_vstd!seq.impl&__2.take.?_pre_post_definition
   :skolemid skolem_internal_vstd!seq.impl&__2.take.?_pre_post_definition
)))
(declare-fun req%vstd!seq.impl&%2.spec_index_range_to. (Dcr Type Dcr Type Poly Poly)
 Bool
)
(declare-const %%global_location_label%%6 Bool)
(assert
 (forall ((A&. Dcr) (A& Type) (J&. Dcr) (J& Type) (spec%param%0! Poly) (spec%param%1!
    Poly
   )
  ) (!
   (= (req%vstd!seq.impl&%2.spec_index_range_to. A&. A& J&. J& spec%param%0! spec%param%1!)
    (=>
     %%global_location_label%%6
     (let
      ((tmp%%$ 0))
      (let
       ((tmp%%$1 (%I spec%param%1!)))
       (let
        ((tmp%%$2 (vstd!seq.Seq.len.? A&. A& spec%param%0!)))
        (and
         (<= tmp%%$ tmp%%$1)
         (<= tmp%%$1 tmp%%$2)
   ))))))
   :pattern ((req%vstd!seq.impl&%2.spec_index_range_to. A&. A& J&. J& spec%param%0! spec%param%1!))
   :qid internal_req__vstd!seq.impl&__2.spec_index_range_to._definition
   :skolemid skolem_internal_req__vstd!seq.impl&__2.spec_index_range_to._definition
)))
(assert
 (fuel_bool_default fuel%vstd!seq.impl&%2.spec_index_range_to.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!seq.impl&%2.spec_index_range_to.)
  (forall ((A&. Dcr) (A& Type) (J&. Dcr) (J& Type) (spec%param%0! Poly) (spec%param%1!
     Poly
    )
   ) (!
    (= (vstd!seq.impl&%2.spec_index_range_to.? A&. A& J&. J& spec%param%0! spec%param%1!)
     (vstd!seq.Seq.subrange.? A&. A& spec%param%0! (I 0) spec%param%1!)
    )
    :pattern ((vstd!seq.impl&%2.spec_index_range_to.? A&. A& J&. J& spec%param%0! spec%param%1!))
    :qid internal_vstd!seq.impl&__2.spec_index_range_to.?_definition
    :skolemid skolem_internal_vstd!seq.impl&__2.spec_index_range_to.?_definition
))))
(assert
 (forall ((A&. Dcr) (A& Type) (J&. Dcr) (J& Type) (spec%param%0! Poly) (spec%param%1!
    Poly
   )
  ) (!
   (=>
    (and
     (has_type spec%param%0! (TYPE%vstd!seq.Seq. A&. A&))
     (has_type spec%param%1! J&)
    )
    (has_type (vstd!seq.impl&%2.spec_index_range_to.? A&. A& J&. J& spec%param%0! spec%param%1!)
     (TYPE%vstd!seq.Seq. A&. A&)
   ))
   :pattern ((vstd!seq.impl&%2.spec_index_range_to.? A&. A& J&. J& spec%param%0! spec%param%1!))
   :qid internal_vstd!seq.impl&__2.spec_index_range_to.?_pre_post_definition
   :skolemid skolem_internal_vstd!seq.impl&__2.spec_index_range_to.?_pre_post_definition
)))
(assert
 (fuel_bool_default fuel%vstd!seq.impl&%2.skip.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!seq.impl&%2.skip.)
  (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
    (= (vstd!seq.impl&%2.skip.? A&. A& spec%param%0! spec%param%1!) (vstd!seq.Seq.subrange.?
      A&. A& spec%param%0! spec%param%1! (I (vstd!seq.Seq.len.? A&. A& spec%param%0!))
    ))
    :pattern ((vstd!seq.impl&%2.skip.? A&. A& spec%param%0! spec%param%1!))
    :qid internal_vstd!seq.impl&__2.skip.?_definition
    :skolemid skolem_internal_vstd!seq.impl&__2.skip.?_definition
))))
(assert
 (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (=>
    (and
     (has_type spec%param%0! (TYPE%vstd!seq.Seq. A&. A&))
     (has_type spec%param%1! INT)
    )
    (has_type (vstd!seq.impl&%2.skip.? A&. A& spec%param%0! spec%param%1!) (TYPE%vstd!seq.Seq.
      A&. A&
   )))
   :pattern ((vstd!seq.impl&%2.skip.? A&. A& spec%param%0! spec%param%1!))
   :qid internal_vstd!seq.impl&__2.skip.?_pre_post_definition
   :skolemid skolem_internal_vstd!seq.impl&__2.skip.?_pre_post_definition
)))
(assert
 (tr_bound%core!clone.Clone. $ NAT)
)
(assert
 (tr_bound%core!marker.Copy. $ NAT)
)
(assert
 (tr_bound%verus_builtin!Integer. $ NAT)
)
(assert
 (forall ((A&. Dcr) (A& Type) (F&. Dcr) (F& Type)) (!
   (=>
    (and
     (sized A&.)
     (tr_bound%core!marker.Tuple. A&. A&)
     (tr_bound%core!ops.function.FnMut. F&. F& A&. A&)
    )
    (tr_bound%core!ops.function.FnOnce. $ (MUTREF F&. F&) A&. A&)
   )
   :pattern ((tr_bound%core!ops.function.FnOnce. $ (MUTREF F&. F&) A&. A&))
   :qid internal_core__ops__function__impls__impl&__4_trait_impl_definition
   :skolemid skolem_internal_core__ops__function__impls__impl&__4_trait_impl_definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!function.axiom_fn_mut_call_requires.)
  (forall ((Args&. Dcr) (Args& Type) (F&. Dcr) (F& Type) (f! Poly) (args! Poly)) (!
    (=>
     (and
      (has_type f! (MUTREF F&. F&))
      (has_type args! Args&)
     )
     (=>
      (and
       (and
        (and
         (and
          (sized Args&.)
          (sized F&.)
         )
         (tr_bound%core!marker.Tuple. Args&. Args&)
        )
        (tr_bound%core!ops.function.FnMut. F&. F& Args&. Args&)
       )
       (closure_req F& Args&. Args& (mut_ref_current% f!) args!)
      )
      (closure_req (MUTREF F&. F&) Args&. Args& f! args!)
    ))
    :pattern ((closure_req (MUTREF F&. F&) Args&. Args& f! args!))
    :qid user_vstd__function__axiom_fn_mut_call_requires_0
    :skolemid skolem_user_vstd__function__axiom_fn_mut_call_requires_0
))))
(assert
 (=>
  (fuel_bool fuel%vstd!function.axiom_fn_mut_call_ensures.)
  (forall ((Args&. Dcr) (Args& Type) (F&. Dcr) (F& Type) (f! Poly) (args! Poly) (output!
     Poly
    )
   ) (!
    (=>
     (and
      (has_type f! (MUTREF F&. F&))
      (has_type args! Args&)
      (has_type output! (proj%core!ops.function.FnOnce./Output F&. F& Args&. Args&))
     )
     (=>
      (and
       (and
        (and
         (and
          (sized Args&.)
          (sized F&.)
         )
         (tr_bound%core!marker.Tuple. Args&. Args&)
        )
        (tr_bound%core!ops.function.FnMut. F&. F& Args&. Args&)
       )
       (closure_ens (MUTREF F&. F&) Args&. Args& f! args! output!)
      )
      (closure_ens F& Args&. Args& (mut_ref_current% f!) args! output!)
    ))
    :pattern ((closure_ens (MUTREF F&. F&) Args&. Args& f! args! output!))
    :qid user_vstd__function__axiom_fn_mut_call_ensures_0
    :skolemid skolem_user_vstd__function__axiom_fn_mut_call_ensures_0
))))
(declare-fun req%vstd!seq_lib.impl&%0.drop_first. (Dcr Type Poly) Bool)
(declare-const %%global_location_label%%7 Bool)
(assert
 (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly)) (!
   (= (req%vstd!seq_lib.impl&%0.drop_first. A&. A& spec%param%0!) (=>
     %%global_location_label%%7
     (>= (vstd!seq.Seq.len.? A&. A& spec%param%0!) 1)
   ))
   :pattern ((req%vstd!seq_lib.impl&%0.drop_first. A&. A& spec%param%0!))
   :qid internal_req__vstd!seq_lib.impl&__0.drop_first._definition
   :skolemid skolem_internal_req__vstd!seq_lib.impl&__0.drop_first._definition
)))
(assert
 (fuel_bool_default fuel%vstd!seq_lib.impl&%0.drop_first.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!seq_lib.impl&%0.drop_first.)
  (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly)) (!
    (= (vstd!seq_lib.impl&%0.drop_first.? A&. A& spec%param%0!) (vstd!seq.Seq.subrange.?
      A&. A& spec%param%0! (I 1) (I (vstd!seq.Seq.len.? A&. A& spec%param%0!))
    ))
    :pattern ((vstd!seq_lib.impl&%0.drop_first.? A&. A& spec%param%0!))
    :qid internal_vstd!seq_lib.impl&__0.drop_first.?_definition
    :skolemid skolem_internal_vstd!seq_lib.impl&__0.drop_first.?_definition
))))
(assert
 (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly)) (!
   (=>
    (has_type spec%param%0! (TYPE%vstd!seq.Seq. A&. A&))
    (has_type (vstd!seq_lib.impl&%0.drop_first.? A&. A& spec%param%0!) (TYPE%vstd!seq.Seq.
      A&. A&
   )))
   :pattern ((vstd!seq_lib.impl&%0.drop_first.? A&. A& spec%param%0!))
   :qid internal_vstd!seq_lib.impl&__0.drop_first.?_pre_post_definition
   :skolemid skolem_internal_vstd!seq_lib.impl&__0.drop_first.?_pre_post_definition
)))
(declare-const fuel_nat%vstd!arithmetic.power.pow. Fuel)
(assert
 (forall ((spec%param%0! Poly) (spec%param%1! Poly) (fuel% Fuel)) (!
   (= (vstd!arithmetic.power.rec%pow.? spec%param%0! spec%param%1! fuel%) (vstd!arithmetic.power.rec%pow.?
     spec%param%0! spec%param%1! zero
   ))
   :pattern ((vstd!arithmetic.power.rec%pow.? spec%param%0! spec%param%1! fuel%))
   :qid internal_vstd!arithmetic.power.pow._fuel_to_zero_definition
   :skolemid skolem_internal_vstd!arithmetic.power.pow._fuel_to_zero_definition
)))
(assert
 (forall ((spec%param%0! Poly) (spec%param%1! Poly) (fuel% Fuel)) (!
   (=>
    (and
     (has_type spec%param%0! INT)
     (has_type spec%param%1! NAT)
    )
    (= (vstd!arithmetic.power.rec%pow.? spec%param%0! spec%param%1! (succ fuel%)) (ite
      (= (%I spec%param%1!) 0)
      1
      (Mul (%I spec%param%0!) (vstd!arithmetic.power.rec%pow.? spec%param%0! (I (nClip (Sub
           (%I spec%param%1!) 1
         ))
        ) fuel%
   )))))
   :pattern ((vstd!arithmetic.power.rec%pow.? spec%param%0! spec%param%1! (succ fuel%)))
   :qid internal_vstd!arithmetic.power.pow._fuel_to_body_definition
   :skolemid skolem_internal_vstd!arithmetic.power.pow._fuel_to_body_definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!arithmetic.power.pow.)
  (forall ((spec%param%0! Poly) (spec%param%1! Poly)) (!
    (=>
     (and
      (has_type spec%param%0! INT)
      (has_type spec%param%1! NAT)
     )
     (= (vstd!arithmetic.power.pow.? spec%param%0! spec%param%1!) (vstd!arithmetic.power.rec%pow.?
       spec%param%0! spec%param%1! (succ fuel_nat%vstd!arithmetic.power.pow.)
    )))
    :pattern ((vstd!arithmetic.power.pow.? spec%param%0! spec%param%1!))
    :qid internal_vstd!arithmetic.power.pow.?_definition
    :skolemid skolem_internal_vstd!arithmetic.power.pow.?_definition
))))
(declare-fun req%vstd!arithmetic.power.lemma_pow_positive. (Int Int) Bool)
(declare-const %%global_location_label%%8 Bool)
(assert
 (forall ((b! Int) (e! Int)) (!
   (= (req%vstd!arithmetic.power.lemma_pow_positive. b! e!) (=>
     %%global_location_label%%8
     (> b! 0)
   ))
   :pattern ((req%vstd!arithmetic.power.lemma_pow_positive. b! e!))
   :qid internal_req__vstd!arithmetic.power.lemma_pow_positive._definition
   :skolemid skolem_internal_req__vstd!arithmetic.power.lemma_pow_positive._definition
)))
(declare-fun ens%vstd!arithmetic.power.lemma_pow_positive. (Int Int) Bool)
(assert
 (forall ((b! Int) (e! Int)) (!
   (= (ens%vstd!arithmetic.power.lemma_pow_positive. b! e!) (< 0 (vstd!arithmetic.power.pow.?
      (I b!) (I e!)
   )))
   :pattern ((ens%vstd!arithmetic.power.lemma_pow_positive. b! e!))
   :qid internal_ens__vstd!arithmetic.power.lemma_pow_positive._definition
   :skolemid skolem_internal_ens__vstd!arithmetic.power.lemma_pow_positive._definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!arithmetic.power.lemma_pow_positive.)
  (forall ((b! Poly) (e! Poly)) (!
    (=>
     (and
      (has_type b! INT)
      (has_type e! NAT)
     )
     (=>
      (> (%I b!) 0)
      (< 0 (vstd!arithmetic.power.pow.? b! e!))
    ))
    :pattern ((vstd!arithmetic.power.pow.? b! e!))
    :qid user_vstd__arithmetic__power__lemma_pow_positive_0
    :skolemid skolem_user_vstd__arithmetic__power__lemma_pow_positive_0
))))
(assert
 (tr_bound%core!clone.Clone. $ (UINT 8))
)
(assert
 (tr_bound%core!clone.Clone. $ (UINT 32))
)
(assert
 (tr_bound%core!clone.Clone. $ (UINT 64))
)
(assert
 (tr_bound%core!clone.Clone. $ USIZE)
)
(assert
 (tr_bound%core!marker.Copy. $ USIZE)
)
(assert
 (tr_bound%verus_builtin!Integer. $ USIZE)
)
(assert
 (forall ((Self%&. Dcr) (Self%& Type)) (!
   (has_type (vstd!endian.Base.base.? Self%&. Self%&) NAT)
   :pattern ((vstd!endian.Base.base.? Self%&. Self%&))
   :qid internal_vstd!endian.Base.base.?_pre_post_definition
   :skolemid skolem_internal_vstd!endian.Base.base.?_pre_post_definition
)))
(declare-fun ens%vstd!endian.Base.base_min. (Dcr Type) Bool)
(assert
 (forall ((Self%&. Dcr) (Self%& Type)) (!
   (= (ens%vstd!endian.Base.base_min. Self%&. Self%&) (> (%I (vstd!endian.Base.base.? Self%&.
       Self%&
      )
     ) 1
   ))
   :pattern ((ens%vstd!endian.Base.base_min. Self%&. Self%&))
   :qid internal_ens__vstd!endian.Base.base_min._definition
   :skolemid skolem_internal_ens__vstd!endian.Base.base_min._definition
)))
(assert
 (forall ((Self%&. Dcr) (Self%& Type)) (!
   (has_type (vstd!endian.BasePow2.bits.? Self%&. Self%&) NAT)
   :pattern ((vstd!endian.BasePow2.bits.? Self%&. Self%&))
   :qid internal_vstd!endian.BasePow2.bits.?_pre_post_definition
   :skolemid skolem_internal_vstd!endian.BasePow2.bits.?_pre_post_definition
)))
(assert
 (fuel_bool_default fuel%vstd!arithmetic.power2.pow2.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!arithmetic.power2.pow2.)
  (forall ((spec%param%0! Poly)) (!
    (= (vstd!arithmetic.power2.pow2.? spec%param%0!) (nClip (vstd!arithmetic.power.pow.?
       (I 2) spec%param%0!
    )))
    :pattern ((vstd!arithmetic.power2.pow2.? spec%param%0!))
    :qid internal_vstd!arithmetic.power2.pow2.?_definition
    :skolemid skolem_internal_vstd!arithmetic.power2.pow2.?_definition
))))
(assert
 (forall ((spec%param%0! Poly)) (!
   (=>
    (has_type spec%param%0! NAT)
    (<= 0 (vstd!arithmetic.power2.pow2.? spec%param%0!))
   )
   :pattern ((vstd!arithmetic.power2.pow2.? spec%param%0!))
   :qid internal_vstd!arithmetic.power2.pow2.?_pre_post_definition
   :skolemid skolem_internal_vstd!arithmetic.power2.pow2.?_pre_post_definition
)))
(declare-fun ens%vstd!endian.BasePow2.bits_to_base. (Dcr Type) Bool)
(assert
 (forall ((Self%&. Dcr) (Self%& Type)) (!
   (= (ens%vstd!endian.BasePow2.bits_to_base. Self%&. Self%&) (and
     (> (%I (vstd!endian.BasePow2.bits.? Self%&. Self%&)) 1)
     (= (%I (vstd!endian.Base.base.? Self%&. Self%&)) (vstd!arithmetic.power2.pow2.? (vstd!endian.BasePow2.bits.?
        Self%&. Self%&
   )))))
   :pattern ((ens%vstd!endian.BasePow2.bits_to_base. Self%&. Self%&))
   :qid internal_ens__vstd!endian.BasePow2.bits_to_base._definition
   :skolemid skolem_internal_ens__vstd!endian.BasePow2.bits_to_base._definition
)))
(declare-fun ens%vstd!endian.CompatibleSmallerBaseFor.compatible. (Dcr Type Dcr Type)
 Bool
)
(assert
 (forall ((Self%&. Dcr) (Self%& Type) (BIG&. Dcr) (BIG& Type)) (!
   (= (ens%vstd!endian.CompatibleSmallerBaseFor.compatible. Self%&. Self%& BIG&. BIG&)
    (and
     (> (%I (vstd!endian.BasePow2.bits.? BIG&. BIG&)) (%I (vstd!endian.BasePow2.bits.? Self%&.
        Self%&
     )))
     (= (EucMod (%I (vstd!endian.BasePow2.bits.? BIG&. BIG&)) (%I (vstd!endian.BasePow2.bits.?
         Self%&. Self%&
       ))
      ) 0
   )))
   :pattern ((ens%vstd!endian.CompatibleSmallerBaseFor.compatible. Self%&. Self%& BIG&.
     BIG&
   ))
   :qid internal_ens__vstd!endian.CompatibleSmallerBaseFor.compatible._definition
   :skolemid skolem_internal_ens__vstd!endian.CompatibleSmallerBaseFor.compatible._definition
)))
(declare-fun req%vstd!seq.impl&%2.first. (Dcr Type Poly) Bool)
(declare-const %%global_location_label%%9 Bool)
(assert
 (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly)) (!
   (= (req%vstd!seq.impl&%2.first. A&. A& spec%param%0!) (=>
     %%global_location_label%%9
     (< 0 (vstd!seq.Seq.len.? A&. A& spec%param%0!))
   ))
   :pattern ((req%vstd!seq.impl&%2.first. A&. A& spec%param%0!))
   :qid internal_req__vstd!seq.impl&__2.first._definition
   :skolemid skolem_internal_req__vstd!seq.impl&__2.first._definition
)))
(assert
 (fuel_bool_default fuel%vstd!seq.impl&%2.first.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!seq.impl&%2.first.)
  (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly)) (!
    (= (vstd!seq.impl&%2.first.? A&. A& spec%param%0!) (vstd!seq.Seq.index.? A&. A& spec%param%0!
      (I 0)
    ))
    :pattern ((vstd!seq.impl&%2.first.? A&. A& spec%param%0!))
    :qid internal_vstd!seq.impl&__2.first.?_definition
    :skolemid skolem_internal_vstd!seq.impl&__2.first.?_definition
))))
(assert
 (forall ((A&. Dcr) (A& Type) (spec%param%0! Poly)) (!
   (=>
    (has_type spec%param%0! (TYPE%vstd!seq.Seq. A&. A&))
    (has_type (vstd!seq.impl&%2.first.? A&. A& spec%param%0!) A&)
   )
   :pattern ((vstd!seq.impl&%2.first.? A&. A& spec%param%0!))
   :qid internal_vstd!seq.impl&__2.first.?_pre_post_definition
   :skolemid skolem_internal_vstd!seq.impl&__2.first.?_pre_post_definition
)))
(assert
 (fuel_bool_default fuel%vstd!seq_lib.impl&%0.map.)
)
(declare-fun %%lambda%%0 (Type Dcr Type Poly %%Function%%) %%Function%%)
(assert
 (forall ((%%x%% Type) (%%hole%%0 Type) (%%hole%%1 Dcr) (%%hole%%2 Type) (%%hole%%3 Poly)
   (%%hole%%4 %%Function%%) (i$ Poly)
  ) (!
   (=>
    (has_type i$ %%hole%%0)
    (= (%%apply%%0 (mk_fun %%x%% (%%lambda%%0 %%hole%%0 %%hole%%1 %%hole%%2 %%hole%%3 %%hole%%4))
      i$
     ) (%%apply%%1 %%hole%%4 i$ (vstd!seq.Seq.index.? %%hole%%1 %%hole%%2 %%hole%%3 i$))
   ))
   :pattern ((%%apply%%0 (mk_fun %%x%% (%%lambda%%0 %%hole%%0 %%hole%%1 %%hole%%2 %%hole%%3
       %%hole%%4
      )
     ) i$
)))))
(assert
 (=>
  (fuel_bool fuel%vstd!seq_lib.impl&%0.map.)
  (forall ((A&. Dcr) (A& Type) (B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1!
     Poly
    )
   ) (!
    (= (vstd!seq_lib.impl&%0.map.? A&. A& B&. B& spec%param%0! spec%param%1!) (vstd!seq.Seq.new.?
      B&. B& (I (vstd!seq.Seq.len.? A&. A& spec%param%0!)) (Poly%fun%1. (mk_fun (TYPE%fun%1.
         $ INT B&. B&
        ) (%%lambda%%0 INT A&. A& spec%param%0! (%Poly%fun%2. spec%param%1!))
    ))))
    :pattern ((vstd!seq_lib.impl&%0.map.? A&. A& B&. B& spec%param%0! spec%param%1!))
    :qid internal_vstd!seq_lib.impl&__0.map.?_definition
    :skolemid skolem_internal_vstd!seq_lib.impl&__0.map.?_definition
))))
(assert
 (forall ((A&. Dcr) (A& Type) (B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1!
    Poly
   )
  ) (!
   (=>
    (and
     (has_type spec%param%0! (TYPE%vstd!seq.Seq. A&. A&))
     (has_type spec%param%1! (TYPE%fun%2. $ INT A&. A& B&. B&))
    )
    (has_type (vstd!seq_lib.impl&%0.map.? A&. A& B&. B& spec%param%0! spec%param%1!) (
      TYPE%vstd!seq.Seq. B&. B&
   )))
   :pattern ((vstd!seq_lib.impl&%0.map.? A&. A& B&. B& spec%param%0! spec%param%1!))
   :qid internal_vstd!seq_lib.impl&__0.map.?_pre_post_definition
   :skolemid skolem_internal_vstd!seq_lib.impl&__0.map.?_pre_post_definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.base_upper_bound_excl.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.base_upper_bound_excl.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
    (= (vstd!endian.base_upper_bound_excl.? B&. B& spec%param%0!) (vstd!arithmetic.power.pow.?
      (vstd!endian.Base.base.? B&. B&) spec%param%0!
    ))
    :pattern ((vstd!endian.base_upper_bound_excl.? B&. B& spec%param%0!))
    :qid internal_vstd!endian.base_upper_bound_excl.?_definition
    :skolemid skolem_internal_vstd!endian.base_upper_bound_excl.?_definition
))))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.in_bounds.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.in_bounds.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
    (= (vstd!endian.impl&%0.in_bounds.? B&. B& spec%param%0!) (forall ((i$ Poly)) (!
       (=>
        (has_type i$ INT)
        (=>
         (let
          ((tmp%%$ 0))
          (let
           ((tmp%%$1 (%I i$)))
           (let
            ((tmp%%$2 (vstd!seq.Seq.len.? $ INT spec%param%0!)))
            (and
             (<= tmp%%$ tmp%%$1)
             (< tmp%%$1 tmp%%$2)
         ))))
         (let
          ((tmp%%$ 0))
          (let
           ((tmp%%$4 (%I (vstd!seq.Seq.index.? $ INT spec%param%0! i$))))
           (let
            ((tmp%%$5 (%I (vstd!endian.Base.base.? B&. B&))))
            (and
             (<= tmp%%$ tmp%%$4)
             (< tmp%%$4 tmp%%$5)
       ))))))
       :pattern ((vstd!seq.Seq.index.? $ INT spec%param%0! i$))
       :qid user_vstd__endian__EndianNat__in_bounds_0
       :skolemid skolem_user_vstd__endian__EndianNat__in_bounds_0
    )))
    :pattern ((vstd!endian.impl&%0.in_bounds.? B&. B& spec%param%0!))
    :qid internal_vstd!endian.impl&__0.in_bounds.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.in_bounds.?_definition
))))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.wf.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.wf.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
    (= (vstd!endian.impl&%0.wf.? B&. B& spec%param%0!) (vstd!endian.impl&%0.in_bounds.?
      B&. B& (Poly%vstd!seq.Seq<int.>. (vstd!endian.EndianNat./EndianNat/digits (%Poly%vstd!endian.EndianNat.
         spec%param%0!
    )))))
    :pattern ((vstd!endian.impl&%0.wf.? B&. B& spec%param%0!))
    :qid internal_vstd!endian.impl&__0.wf.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.wf.?_definition
))))
(declare-fun req%vstd!endian.impl&%0.new. (Dcr Type Poly Poly) Bool)
(declare-const %%global_location_label%%10 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (= (req%vstd!endian.impl&%0.new. B&. B& spec%param%0! spec%param%1!) (=>
     %%global_location_label%%10
     (vstd!endian.impl&%0.in_bounds.? B&. B& spec%param%1!)
   ))
   :pattern ((req%vstd!endian.impl&%0.new. B&. B& spec%param%0! spec%param%1!))
   :qid internal_req__vstd!endian.impl&__0.new._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.new._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.new.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.new.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
    (= (vstd!endian.impl&%0.new.? B&. B& spec%param%0! spec%param%1!) (vstd!endian.EndianNat./EndianNat
      (%Poly%vstd!endian.Endian. spec%param%0!) (%Poly%vstd!seq.Seq<int.>. spec%param%1!)
      (%Poly%core!marker.PhantomData. (Poly%core!marker.PhantomData. core!marker.PhantomData./PhantomData))
    ))
    :pattern ((vstd!endian.impl&%0.new.? B&. B& spec%param%0! spec%param%1!))
    :qid internal_vstd!endian.impl&__0.new.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.new.?_definition
))))
(declare-fun req%vstd!endian.impl&%0.new_default. (Dcr Type Poly) Bool)
(declare-const %%global_location_label%%11 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
   (= (req%vstd!endian.impl&%0.new_default. B&. B& spec%param%0!) (=>
     %%global_location_label%%11
     (vstd!endian.impl&%0.in_bounds.? B&. B& spec%param%0!)
   ))
   :pattern ((req%vstd!endian.impl&%0.new_default. B&. B& spec%param%0!))
   :qid internal_req__vstd!endian.impl&__0.new_default._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.new_default._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.new_default.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.new_default.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
    (= (vstd!endian.impl&%0.new_default.? B&. B& spec%param%0!) (vstd!endian.EndianNat./EndianNat
      (%Poly%vstd!endian.Endian. (Poly%vstd!endian.Endian. (vstd!endian.endianness.? (I 0))))
      (%Poly%vstd!seq.Seq<int.>. spec%param%0!) (%Poly%core!marker.PhantomData. (Poly%core!marker.PhantomData.
        core!marker.PhantomData./PhantomData
    ))))
    :pattern ((vstd!endian.impl&%0.new_default.? B&. B& spec%param%0!))
    :qid internal_vstd!endian.impl&__0.new_default.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.new_default.?_definition
))))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.len.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.len.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
    (= (vstd!endian.impl&%0.len.? B&. B& spec%param%0!) (vstd!seq.Seq.len.? $ INT (Poly%vstd!seq.Seq<int.>.
       (vstd!endian.EndianNat./EndianNat/digits (%Poly%vstd!endian.EndianNat. spec%param%0!))
    )))
    :pattern ((vstd!endian.impl&%0.len.? B&. B& spec%param%0!))
    :qid internal_vstd!endian.impl&__0.len.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.len.?_definition
))))
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
   (=>
    (has_type spec%param%0! (TYPE%vstd!endian.EndianNat. B&. B&))
    (<= 0 (vstd!endian.impl&%0.len.? B&. B& spec%param%0!))
   )
   :pattern ((vstd!endian.impl&%0.len.? B&. B& spec%param%0!))
   :qid internal_vstd!endian.impl&__0.len.?_pre_post_definition
   :skolemid skolem_internal_vstd!endian.impl&__0.len.?_pre_post_definition
)))
(declare-fun req%vstd!endian.impl&%0.index. (Dcr Type Poly Poly) Bool)
(declare-const %%global_location_label%%12 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (= (req%vstd!endian.impl&%0.index. B&. B& spec%param%0! spec%param%1!) (=>
     %%global_location_label%%12
     (let
      ((tmp%%$ 0))
      (let
       ((tmp%%$1 (%I spec%param%1!)))
       (let
        ((tmp%%$2 (vstd!seq.Seq.len.? $ INT (Poly%vstd!seq.Seq<int.>. (vstd!endian.EndianNat./EndianNat/digits
             (%Poly%vstd!endian.EndianNat. spec%param%0!)
        )))))
        (and
         (<= tmp%%$ tmp%%$1)
         (< tmp%%$1 tmp%%$2)
   ))))))
   :pattern ((req%vstd!endian.impl&%0.index. B&. B& spec%param%0! spec%param%1!))
   :qid internal_req__vstd!endian.impl&__0.index._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.index._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.index.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.index.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
    (= (vstd!endian.impl&%0.index.? B&. B& spec%param%0! spec%param%1!) (%I (vstd!seq.Seq.index.?
       $ INT (Poly%vstd!seq.Seq<int.>. (vstd!endian.EndianNat./EndianNat/digits (%Poly%vstd!endian.EndianNat.
          spec%param%0!
        ))
       ) spec%param%1!
    )))
    :pattern ((vstd!endian.impl&%0.index.? B&. B& spec%param%0! spec%param%1!))
    :qid internal_vstd!endian.impl&__0.index.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.index.?_definition
))))
(declare-fun req%vstd!endian.impl&%0.spec_index. (Dcr Type Poly Poly) Bool)
(declare-const %%global_location_label%%13 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (= (req%vstd!endian.impl&%0.spec_index. B&. B& spec%param%0! spec%param%1!) (=>
     %%global_location_label%%13
     (let
      ((tmp%%$ 0))
      (let
       ((tmp%%$1 (%I spec%param%1!)))
       (let
        ((tmp%%$2 (vstd!seq.Seq.len.? $ INT (Poly%vstd!seq.Seq<int.>. (vstd!endian.EndianNat./EndianNat/digits
             (%Poly%vstd!endian.EndianNat. spec%param%0!)
        )))))
        (and
         (<= tmp%%$ tmp%%$1)
         (< tmp%%$1 tmp%%$2)
   ))))))
   :pattern ((req%vstd!endian.impl&%0.spec_index. B&. B& spec%param%0! spec%param%1!))
   :qid internal_req__vstd!endian.impl&__0.spec_index._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.spec_index._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.spec_index.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.spec_index.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
    (= (vstd!endian.impl&%0.spec_index.? B&. B& spec%param%0! spec%param%1!) (vstd!endian.impl&%0.index.?
      B&. B& spec%param%0! spec%param%1!
    ))
    :pattern ((vstd!endian.impl&%0.spec_index.? B&. B& spec%param%0! spec%param%1!))
    :qid internal_vstd!endian.impl&__0.spec_index.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.spec_index.?_definition
))))
(declare-fun req%vstd!endian.impl&%0.first. (Dcr Type Poly) Bool)
(declare-const %%global_location_label%%14 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
   (= (req%vstd!endian.impl&%0.first. B&. B& spec%param%0!) (=>
     %%global_location_label%%14
     (> (vstd!seq.Seq.len.? $ INT (Poly%vstd!seq.Seq<int.>. (vstd!endian.EndianNat./EndianNat/digits
         (%Poly%vstd!endian.EndianNat. spec%param%0!)
       ))
      ) 0
   )))
   :pattern ((req%vstd!endian.impl&%0.first. B&. B& spec%param%0!))
   :qid internal_req__vstd!endian.impl&__0.first._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.first._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.first.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.first.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
    (= (vstd!endian.impl&%0.first.? B&. B& spec%param%0!) (nClip (%I (vstd!seq.impl&%2.first.?
        $ INT (Poly%vstd!seq.Seq<int.>. (vstd!endian.EndianNat./EndianNat/digits (%Poly%vstd!endian.EndianNat.
           spec%param%0!
    )))))))
    :pattern ((vstd!endian.impl&%0.first.? B&. B& spec%param%0!))
    :qid internal_vstd!endian.impl&__0.first.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.first.?_definition
))))
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
   (=>
    (has_type spec%param%0! (TYPE%vstd!endian.EndianNat. B&. B&))
    (<= 0 (vstd!endian.impl&%0.first.? B&. B& spec%param%0!))
   )
   :pattern ((vstd!endian.impl&%0.first.? B&. B& spec%param%0!))
   :qid internal_vstd!endian.impl&__0.first.?_pre_post_definition
   :skolemid skolem_internal_vstd!endian.impl&__0.first.?_pre_post_definition
)))
(declare-fun req%vstd!endian.impl&%0.last. (Dcr Type Poly) Bool)
(declare-const %%global_location_label%%15 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
   (= (req%vstd!endian.impl&%0.last. B&. B& spec%param%0!) (=>
     %%global_location_label%%15
     (> (vstd!seq.Seq.len.? $ INT (Poly%vstd!seq.Seq<int.>. (vstd!endian.EndianNat./EndianNat/digits
         (%Poly%vstd!endian.EndianNat. spec%param%0!)
       ))
      ) 0
   )))
   :pattern ((req%vstd!endian.impl&%0.last. B&. B& spec%param%0!))
   :qid internal_req__vstd!endian.impl&__0.last._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.last._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.last.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.last.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
    (= (vstd!endian.impl&%0.last.? B&. B& spec%param%0!) (nClip (%I (vstd!seq.Seq.last.?
        $ INT (Poly%vstd!seq.Seq<int.>. (vstd!endian.EndianNat./EndianNat/digits (%Poly%vstd!endian.EndianNat.
           spec%param%0!
    )))))))
    :pattern ((vstd!endian.impl&%0.last.? B&. B& spec%param%0!))
    :qid internal_vstd!endian.impl&__0.last.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.last.?_definition
))))
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
   (=>
    (has_type spec%param%0! (TYPE%vstd!endian.EndianNat. B&. B&))
    (<= 0 (vstd!endian.impl&%0.last.? B&. B& spec%param%0!))
   )
   :pattern ((vstd!endian.impl&%0.last.? B&. B& spec%param%0!))
   :qid internal_vstd!endian.impl&__0.last.?_pre_post_definition
   :skolemid skolem_internal_vstd!endian.impl&__0.last.?_pre_post_definition
)))
(declare-fun req%vstd!endian.impl&%0.least. (Dcr Type Poly) Bool)
(declare-const %%global_location_label%%16 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
   (= (req%vstd!endian.impl&%0.least. B&. B& spec%param%0!) (=>
     %%global_location_label%%16
     (> (vstd!seq.Seq.len.? $ INT (Poly%vstd!seq.Seq<int.>. (vstd!endian.EndianNat./EndianNat/digits
         (%Poly%vstd!endian.EndianNat. spec%param%0!)
       ))
      ) 0
   )))
   :pattern ((req%vstd!endian.impl&%0.least. B&. B& spec%param%0!))
   :qid internal_req__vstd!endian.impl&__0.least._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.least._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.least.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.least.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
    (= (vstd!endian.impl&%0.least.? B&. B& spec%param%0!) (ite
      (is-vstd!endian.Endian./Little (vstd!endian.EndianNat./EndianNat/endian (%Poly%vstd!endian.EndianNat.
         spec%param%0!
      )))
      (vstd!endian.impl&%0.first.? B&. B& spec%param%0!)
      (vstd!endian.impl&%0.last.? B&. B& spec%param%0!)
    ))
    :pattern ((vstd!endian.impl&%0.least.? B&. B& spec%param%0!))
    :qid internal_vstd!endian.impl&__0.least.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.least.?_definition
))))
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
   (=>
    (has_type spec%param%0! (TYPE%vstd!endian.EndianNat. B&. B&))
    (<= 0 (vstd!endian.impl&%0.least.? B&. B& spec%param%0!))
   )
   :pattern ((vstd!endian.impl&%0.least.? B&. B& spec%param%0!))
   :qid internal_vstd!endian.impl&__0.least.?_pre_post_definition
   :skolemid skolem_internal_vstd!endian.impl&__0.least.?_pre_post_definition
)))
(declare-fun req%vstd!endian.impl&%0.most. (Dcr Type Poly) Bool)
(declare-const %%global_location_label%%17 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
   (= (req%vstd!endian.impl&%0.most. B&. B& spec%param%0!) (=>
     %%global_location_label%%17
     (> (vstd!seq.Seq.len.? $ INT (Poly%vstd!seq.Seq<int.>. (vstd!endian.EndianNat./EndianNat/digits
         (%Poly%vstd!endian.EndianNat. spec%param%0!)
       ))
      ) 0
   )))
   :pattern ((req%vstd!endian.impl&%0.most. B&. B& spec%param%0!))
   :qid internal_req__vstd!endian.impl&__0.most._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.most._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.most.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.most.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
    (= (vstd!endian.impl&%0.most.? B&. B& spec%param%0!) (ite
      (is-vstd!endian.Endian./Little (vstd!endian.EndianNat./EndianNat/endian (%Poly%vstd!endian.EndianNat.
         spec%param%0!
      )))
      (vstd!endian.impl&%0.last.? B&. B& spec%param%0!)
      (vstd!endian.impl&%0.first.? B&. B& spec%param%0!)
    ))
    :pattern ((vstd!endian.impl&%0.most.? B&. B& spec%param%0!))
    :qid internal_vstd!endian.impl&__0.most.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.most.?_definition
))))
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
   (=>
    (has_type spec%param%0! (TYPE%vstd!endian.EndianNat. B&. B&))
    (<= 0 (vstd!endian.impl&%0.most.? B&. B& spec%param%0!))
   )
   :pattern ((vstd!endian.impl&%0.most.? B&. B& spec%param%0!))
   :qid internal_vstd!endian.impl&__0.most.?_pre_post_definition
   :skolemid skolem_internal_vstd!endian.impl&__0.most.?_pre_post_definition
)))
(declare-fun req%vstd!endian.impl&%0.skip. (Dcr Type Poly Poly) Bool)
(declare-const %%global_location_label%%18 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (= (req%vstd!endian.impl&%0.skip. B&. B& spec%param%0! spec%param%1!) (=>
     %%global_location_label%%18
     (let
      ((tmp%%$ 0))
      (let
       ((tmp%%$1 (%I spec%param%1!)))
       (let
        ((tmp%%$2 (vstd!seq.Seq.len.? $ INT (Poly%vstd!seq.Seq<int.>. (vstd!endian.EndianNat./EndianNat/digits
             (%Poly%vstd!endian.EndianNat. spec%param%0!)
        )))))
        (and
         (<= tmp%%$ tmp%%$1)
         (<= tmp%%$1 tmp%%$2)
   ))))))
   :pattern ((req%vstd!endian.impl&%0.skip. B&. B& spec%param%0! spec%param%1!))
   :qid internal_req__vstd!endian.impl&__0.skip._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.skip._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.skip.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.skip.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
    (= (vstd!endian.impl&%0.skip.? B&. B& spec%param%0! spec%param%1!) (vstd!endian.EndianNat./EndianNat
      (%Poly%vstd!endian.Endian. (Poly%vstd!endian.Endian. (vstd!endian.EndianNat./EndianNat/endian
         (%Poly%vstd!endian.EndianNat. spec%param%0!)
       ))
      ) (%Poly%vstd!seq.Seq<int.>. (vstd!seq.Seq.subrange.? $ INT (Poly%vstd!seq.Seq<int.>.
         (vstd!endian.EndianNat./EndianNat/digits (%Poly%vstd!endian.EndianNat. spec%param%0!))
        ) spec%param%1! (I (vstd!seq.Seq.len.? $ INT (Poly%vstd!seq.Seq<int.>. (vstd!endian.EndianNat./EndianNat/digits
            (%Poly%vstd!endian.EndianNat. spec%param%0!)
       )))))
      ) (%Poly%core!marker.PhantomData. (Poly%core!marker.PhantomData. (vstd!endian.EndianNat./EndianNat/phantom
         (%Poly%vstd!endian.EndianNat. spec%param%0!)
    )))))
    :pattern ((vstd!endian.impl&%0.skip.? B&. B& spec%param%0! spec%param%1!))
    :qid internal_vstd!endian.impl&__0.skip.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.skip.?_definition
))))
(declare-fun req%vstd!endian.impl&%0.take. (Dcr Type Poly Poly) Bool)
(declare-const %%global_location_label%%19 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (= (req%vstd!endian.impl&%0.take. B&. B& spec%param%0! spec%param%1!) (=>
     %%global_location_label%%19
     (let
      ((tmp%%$ 0))
      (let
       ((tmp%%$1 (%I spec%param%1!)))
       (let
        ((tmp%%$2 (vstd!seq.Seq.len.? $ INT (Poly%vstd!seq.Seq<int.>. (vstd!endian.EndianNat./EndianNat/digits
             (%Poly%vstd!endian.EndianNat. spec%param%0!)
        )))))
        (and
         (<= tmp%%$ tmp%%$1)
         (<= tmp%%$1 tmp%%$2)
   ))))))
   :pattern ((req%vstd!endian.impl&%0.take. B&. B& spec%param%0! spec%param%1!))
   :qid internal_req__vstd!endian.impl&__0.take._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.take._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.take.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.take.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
    (= (vstd!endian.impl&%0.take.? B&. B& spec%param%0! spec%param%1!) (vstd!endian.EndianNat./EndianNat
      (%Poly%vstd!endian.Endian. (Poly%vstd!endian.Endian. (vstd!endian.EndianNat./EndianNat/endian
         (%Poly%vstd!endian.EndianNat. spec%param%0!)
       ))
      ) (%Poly%vstd!seq.Seq<int.>. (vstd!seq.Seq.subrange.? $ INT (Poly%vstd!seq.Seq<int.>.
         (vstd!endian.EndianNat./EndianNat/digits (%Poly%vstd!endian.EndianNat. spec%param%0!))
        ) (I 0) spec%param%1!
       )
      ) (%Poly%core!marker.PhantomData. (Poly%core!marker.PhantomData. (vstd!endian.EndianNat./EndianNat/phantom
         (%Poly%vstd!endian.EndianNat. spec%param%0!)
    )))))
    :pattern ((vstd!endian.impl&%0.take.? B&. B& spec%param%0! spec%param%1!))
    :qid internal_vstd!endian.impl&__0.take.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.take.?_definition
))))
(declare-fun req%vstd!endian.impl&%0.spec_index_range_from. (Dcr Type Dcr Type Poly
  Poly
 ) Bool
)
(declare-const %%global_location_label%%20 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (N&. Dcr) (N& Type) (spec%param%0! Poly) (spec%param%1!
    Poly
   )
  ) (!
   (= (req%vstd!endian.impl&%0.spec_index_range_from. B&. B& N&. N& spec%param%0! spec%param%1!)
    (=>
     %%global_location_label%%20
     (let
      ((tmp%%$ 0))
      (let
       ((tmp%%$1 (%I spec%param%1!)))
       (let
        ((tmp%%$2 (vstd!seq.Seq.len.? $ INT (Poly%vstd!seq.Seq<int.>. (vstd!endian.EndianNat./EndianNat/digits
             (%Poly%vstd!endian.EndianNat. spec%param%0!)
        )))))
        (and
         (<= tmp%%$ tmp%%$1)
         (<= tmp%%$1 tmp%%$2)
   ))))))
   :pattern ((req%vstd!endian.impl&%0.spec_index_range_from. B&. B& N&. N& spec%param%0!
     spec%param%1!
   ))
   :qid internal_req__vstd!endian.impl&__0.spec_index_range_from._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.spec_index_range_from._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.spec_index_range_from.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.spec_index_range_from.)
  (forall ((B&. Dcr) (B& Type) (N&. Dcr) (N& Type) (spec%param%0! Poly) (spec%param%1!
     Poly
    )
   ) (!
    (= (vstd!endian.impl&%0.spec_index_range_from.? B&. B& N&. N& spec%param%0! spec%param%1!)
     (vstd!endian.impl&%0.skip.? B&. B& spec%param%0! (I (nClip (%I spec%param%1!))))
    )
    :pattern ((vstd!endian.impl&%0.spec_index_range_from.? B&. B& N&. N& spec%param%0!
      spec%param%1!
    ))
    :qid internal_vstd!endian.impl&__0.spec_index_range_from.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.spec_index_range_from.?_definition
))))
(declare-fun req%vstd!endian.impl&%0.spec_index_range_to. (Dcr Type Dcr Type Poly Poly)
 Bool
)
(declare-const %%global_location_label%%21 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (N&. Dcr) (N& Type) (spec%param%0! Poly) (spec%param%1!
    Poly
   )
  ) (!
   (= (req%vstd!endian.impl&%0.spec_index_range_to. B&. B& N&. N& spec%param%0! spec%param%1!)
    (=>
     %%global_location_label%%21
     (let
      ((tmp%%$ 0))
      (let
       ((tmp%%$1 (%I spec%param%1!)))
       (let
        ((tmp%%$2 (vstd!seq.Seq.len.? $ INT (Poly%vstd!seq.Seq<int.>. (vstd!endian.EndianNat./EndianNat/digits
             (%Poly%vstd!endian.EndianNat. spec%param%0!)
        )))))
        (and
         (<= tmp%%$ tmp%%$1)
         (<= tmp%%$1 tmp%%$2)
   ))))))
   :pattern ((req%vstd!endian.impl&%0.spec_index_range_to. B&. B& N&. N& spec%param%0!
     spec%param%1!
   ))
   :qid internal_req__vstd!endian.impl&__0.spec_index_range_to._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.spec_index_range_to._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.spec_index_range_to.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.spec_index_range_to.)
  (forall ((B&. Dcr) (B& Type) (N&. Dcr) (N& Type) (spec%param%0! Poly) (spec%param%1!
     Poly
    )
   ) (!
    (= (vstd!endian.impl&%0.spec_index_range_to.? B&. B& N&. N& spec%param%0! spec%param%1!)
     (vstd!endian.impl&%0.take.? B&. B& spec%param%0! (I (nClip (%I spec%param%1!))))
    )
    :pattern ((vstd!endian.impl&%0.spec_index_range_to.? B&. B& N&. N& spec%param%0! spec%param%1!))
    :qid internal_vstd!endian.impl&__0.spec_index_range_to.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.spec_index_range_to.?_definition
))))
(declare-fun req%vstd!endian.impl&%0.skip_least. (Dcr Type Poly Poly) Bool)
(declare-const %%global_location_label%%22 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (= (req%vstd!endian.impl&%0.skip_least. B&. B& spec%param%0! spec%param%1!) (=>
     %%global_location_label%%22
     (let
      ((tmp%%$ 0))
      (let
       ((tmp%%$1 (%I spec%param%1!)))
       (let
        ((tmp%%$2 (vstd!seq.Seq.len.? $ INT (Poly%vstd!seq.Seq<int.>. (vstd!endian.EndianNat./EndianNat/digits
             (%Poly%vstd!endian.EndianNat. spec%param%0!)
        )))))
        (and
         (<= tmp%%$ tmp%%$1)
         (<= tmp%%$1 tmp%%$2)
   ))))))
   :pattern ((req%vstd!endian.impl&%0.skip_least. B&. B& spec%param%0! spec%param%1!))
   :qid internal_req__vstd!endian.impl&__0.skip_least._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.skip_least._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.skip_least.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.skip_least.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
    (= (vstd!endian.impl&%0.skip_least.? B&. B& spec%param%0! spec%param%1!) (ite
      (is-vstd!endian.Endian./Little (vstd!endian.EndianNat./EndianNat/endian (%Poly%vstd!endian.EndianNat.
         spec%param%0!
      )))
      (vstd!endian.impl&%0.skip.? B&. B& spec%param%0! (I (nClip (%I spec%param%1!))))
      (vstd!endian.impl&%0.take.? B&. B& spec%param%0! (I (nClip (Sub (vstd!endian.impl&%0.len.?
           B&. B& spec%param%0!
          ) (%I spec%param%1!)
    ))))))
    :pattern ((vstd!endian.impl&%0.skip_least.? B&. B& spec%param%0! spec%param%1!))
    :qid internal_vstd!endian.impl&__0.skip_least.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.skip_least.?_definition
))))
(declare-fun req%vstd!endian.impl&%0.skip_most. (Dcr Type Poly Poly) Bool)
(declare-const %%global_location_label%%23 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (= (req%vstd!endian.impl&%0.skip_most. B&. B& spec%param%0! spec%param%1!) (=>
     %%global_location_label%%23
     (let
      ((tmp%%$ 0))
      (let
       ((tmp%%$1 (%I spec%param%1!)))
       (let
        ((tmp%%$2 (vstd!seq.Seq.len.? $ INT (Poly%vstd!seq.Seq<int.>. (vstd!endian.EndianNat./EndianNat/digits
             (%Poly%vstd!endian.EndianNat. spec%param%0!)
        )))))
        (and
         (<= tmp%%$ tmp%%$1)
         (<= tmp%%$1 tmp%%$2)
   ))))))
   :pattern ((req%vstd!endian.impl&%0.skip_most. B&. B& spec%param%0! spec%param%1!))
   :qid internal_req__vstd!endian.impl&__0.skip_most._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.skip_most._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.skip_most.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.skip_most.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
    (= (vstd!endian.impl&%0.skip_most.? B&. B& spec%param%0! spec%param%1!) (ite
      (is-vstd!endian.Endian./Little (vstd!endian.EndianNat./EndianNat/endian (%Poly%vstd!endian.EndianNat.
         spec%param%0!
      )))
      (vstd!endian.impl&%0.take.? B&. B& spec%param%0! (I (nClip (Sub (vstd!endian.impl&%0.len.?
           B&. B& spec%param%0!
          ) (%I spec%param%1!)
      ))))
      (vstd!endian.impl&%0.skip.? B&. B& spec%param%0! (I (nClip (%I spec%param%1!))))
    ))
    :pattern ((vstd!endian.impl&%0.skip_most.? B&. B& spec%param%0! spec%param%1!))
    :qid internal_vstd!endian.impl&__0.skip_most.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.skip_most.?_definition
))))
(declare-fun req%vstd!endian.impl&%0.take_least. (Dcr Type Poly Poly) Bool)
(declare-const %%global_location_label%%24 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (= (req%vstd!endian.impl&%0.take_least. B&. B& spec%param%0! spec%param%1!) (=>
     %%global_location_label%%24
     (let
      ((tmp%%$ 0))
      (let
       ((tmp%%$1 (%I spec%param%1!)))
       (let
        ((tmp%%$2 (vstd!seq.Seq.len.? $ INT (Poly%vstd!seq.Seq<int.>. (vstd!endian.EndianNat./EndianNat/digits
             (%Poly%vstd!endian.EndianNat. spec%param%0!)
        )))))
        (and
         (<= tmp%%$ tmp%%$1)
         (<= tmp%%$1 tmp%%$2)
   ))))))
   :pattern ((req%vstd!endian.impl&%0.take_least. B&. B& spec%param%0! spec%param%1!))
   :qid internal_req__vstd!endian.impl&__0.take_least._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.take_least._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.take_least.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.take_least.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
    (= (vstd!endian.impl&%0.take_least.? B&. B& spec%param%0! spec%param%1!) (ite
      (is-vstd!endian.Endian./Little (vstd!endian.EndianNat./EndianNat/endian (%Poly%vstd!endian.EndianNat.
         spec%param%0!
      )))
      (vstd!endian.impl&%0.take.? B&. B& spec%param%0! (I (nClip (%I spec%param%1!))))
      (vstd!endian.impl&%0.skip.? B&. B& spec%param%0! (I (nClip (Sub (vstd!endian.impl&%0.len.?
           B&. B& spec%param%0!
          ) (%I spec%param%1!)
    ))))))
    :pattern ((vstd!endian.impl&%0.take_least.? B&. B& spec%param%0! spec%param%1!))
    :qid internal_vstd!endian.impl&__0.take_least.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.take_least.?_definition
))))
(declare-fun req%vstd!endian.impl&%0.take_most. (Dcr Type Poly Poly) Bool)
(declare-const %%global_location_label%%25 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (= (req%vstd!endian.impl&%0.take_most. B&. B& spec%param%0! spec%param%1!) (=>
     %%global_location_label%%25
     (let
      ((tmp%%$ 0))
      (let
       ((tmp%%$1 (%I spec%param%1!)))
       (let
        ((tmp%%$2 (vstd!seq.Seq.len.? $ INT (Poly%vstd!seq.Seq<int.>. (vstd!endian.EndianNat./EndianNat/digits
             (%Poly%vstd!endian.EndianNat. spec%param%0!)
        )))))
        (and
         (<= tmp%%$ tmp%%$1)
         (<= tmp%%$1 tmp%%$2)
   ))))))
   :pattern ((req%vstd!endian.impl&%0.take_most. B&. B& spec%param%0! spec%param%1!))
   :qid internal_req__vstd!endian.impl&__0.take_most._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.take_most._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.take_most.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.take_most.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
    (= (vstd!endian.impl&%0.take_most.? B&. B& spec%param%0! spec%param%1!) (ite
      (is-vstd!endian.Endian./Little (vstd!endian.EndianNat./EndianNat/endian (%Poly%vstd!endian.EndianNat.
         spec%param%0!
      )))
      (vstd!endian.impl&%0.skip.? B&. B& spec%param%0! (I (nClip (Sub (vstd!endian.impl&%0.len.?
           B&. B& spec%param%0!
          ) (%I spec%param%1!)
      ))))
      (vstd!endian.impl&%0.take.? B&. B& spec%param%0! (I (nClip (%I spec%param%1!))))
    ))
    :pattern ((vstd!endian.impl&%0.take_most.? B&. B& spec%param%0! spec%param%1!))
    :qid internal_vstd!endian.impl&__0.take_most.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.take_most.?_definition
))))
(declare-fun req%vstd!endian.impl&%0.drop_first. (Dcr Type Poly) Bool)
(declare-const %%global_location_label%%26 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
   (= (req%vstd!endian.impl&%0.drop_first. B&. B& spec%param%0!) (=>
     %%global_location_label%%26
     (> (vstd!endian.impl&%0.len.? B&. B& spec%param%0!) 0)
   ))
   :pattern ((req%vstd!endian.impl&%0.drop_first. B&. B& spec%param%0!))
   :qid internal_req__vstd!endian.impl&__0.drop_first._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.drop_first._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.drop_first.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.drop_first.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
    (= (vstd!endian.impl&%0.drop_first.? B&. B& spec%param%0!) (vstd!endian.EndianNat./EndianNat
      (%Poly%vstd!endian.Endian. (Poly%vstd!endian.Endian. (vstd!endian.EndianNat./EndianNat/endian
         (%Poly%vstd!endian.EndianNat. spec%param%0!)
       ))
      ) (%Poly%vstd!seq.Seq<int.>. (vstd!seq_lib.impl&%0.drop_first.? $ INT (Poly%vstd!seq.Seq<int.>.
         (vstd!endian.EndianNat./EndianNat/digits (%Poly%vstd!endian.EndianNat. spec%param%0!))
       ))
      ) (%Poly%core!marker.PhantomData. (Poly%core!marker.PhantomData. (vstd!endian.EndianNat./EndianNat/phantom
         (%Poly%vstd!endian.EndianNat. spec%param%0!)
    )))))
    :pattern ((vstd!endian.impl&%0.drop_first.? B&. B& spec%param%0!))
    :qid internal_vstd!endian.impl&__0.drop_first.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.drop_first.?_definition
))))
(declare-fun req%vstd!endian.impl&%0.drop_last. (Dcr Type Poly) Bool)
(declare-const %%global_location_label%%27 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
   (= (req%vstd!endian.impl&%0.drop_last. B&. B& spec%param%0!) (=>
     %%global_location_label%%27
     (> (vstd!endian.impl&%0.len.? B&. B& spec%param%0!) 0)
   ))
   :pattern ((req%vstd!endian.impl&%0.drop_last. B&. B& spec%param%0!))
   :qid internal_req__vstd!endian.impl&__0.drop_last._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.drop_last._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.drop_last.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.drop_last.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
    (= (vstd!endian.impl&%0.drop_last.? B&. B& spec%param%0!) (vstd!endian.EndianNat./EndianNat
      (%Poly%vstd!endian.Endian. (Poly%vstd!endian.Endian. (vstd!endian.EndianNat./EndianNat/endian
         (%Poly%vstd!endian.EndianNat. spec%param%0!)
       ))
      ) (%Poly%vstd!seq.Seq<int.>. (vstd!seq_lib.impl&%0.drop_last.? $ INT (Poly%vstd!seq.Seq<int.>.
         (vstd!endian.EndianNat./EndianNat/digits (%Poly%vstd!endian.EndianNat. spec%param%0!))
       ))
      ) (%Poly%core!marker.PhantomData. (Poly%core!marker.PhantomData. (vstd!endian.EndianNat./EndianNat/phantom
         (%Poly%vstd!endian.EndianNat. spec%param%0!)
    )))))
    :pattern ((vstd!endian.impl&%0.drop_last.? B&. B& spec%param%0!))
    :qid internal_vstd!endian.impl&__0.drop_last.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.drop_last.?_definition
))))
(declare-fun req%vstd!endian.impl&%0.drop_least. (Dcr Type Poly) Bool)
(declare-const %%global_location_label%%28 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
   (= (req%vstd!endian.impl&%0.drop_least. B&. B& spec%param%0!) (=>
     %%global_location_label%%28
     (> (vstd!endian.impl&%0.len.? B&. B& spec%param%0!) 0)
   ))
   :pattern ((req%vstd!endian.impl&%0.drop_least. B&. B& spec%param%0!))
   :qid internal_req__vstd!endian.impl&__0.drop_least._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.drop_least._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.drop_least.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.drop_least.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
    (= (vstd!endian.impl&%0.drop_least.? B&. B& spec%param%0!) (ite
      (is-vstd!endian.Endian./Little (vstd!endian.EndianNat./EndianNat/endian (%Poly%vstd!endian.EndianNat.
         spec%param%0!
      )))
      (vstd!endian.impl&%0.drop_first.? B&. B& spec%param%0!)
      (vstd!endian.impl&%0.drop_last.? B&. B& spec%param%0!)
    ))
    :pattern ((vstd!endian.impl&%0.drop_least.? B&. B& spec%param%0!))
    :qid internal_vstd!endian.impl&__0.drop_least.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.drop_least.?_definition
))))
(declare-fun req%vstd!endian.impl&%0.drop_most. (Dcr Type Poly) Bool)
(declare-const %%global_location_label%%29 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
   (= (req%vstd!endian.impl&%0.drop_most. B&. B& spec%param%0!) (=>
     %%global_location_label%%29
     (> (vstd!endian.impl&%0.len.? B&. B& spec%param%0!) 0)
   ))
   :pattern ((req%vstd!endian.impl&%0.drop_most. B&. B& spec%param%0!))
   :qid internal_req__vstd!endian.impl&__0.drop_most._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.drop_most._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.drop_most.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.drop_most.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
    (= (vstd!endian.impl&%0.drop_most.? B&. B& spec%param%0!) (ite
      (is-vstd!endian.Endian./Little (vstd!endian.EndianNat./EndianNat/endian (%Poly%vstd!endian.EndianNat.
         spec%param%0!
      )))
      (vstd!endian.impl&%0.drop_last.? B&. B& spec%param%0!)
      (vstd!endian.impl&%0.drop_first.? B&. B& spec%param%0!)
    ))
    :pattern ((vstd!endian.impl&%0.drop_most.? B&. B& spec%param%0!))
    :qid internal_vstd!endian.impl&__0.drop_most.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.drop_most.?_definition
))))
(declare-fun req%vstd!endian.impl&%0.append. (Dcr Type Poly Poly) Bool)
(declare-const %%global_location_label%%30 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (= (req%vstd!endian.impl&%0.append. B&. B& spec%param%0! spec%param%1!) (=>
     %%global_location_label%%30
     (= (vstd!endian.EndianNat./EndianNat/endian (%Poly%vstd!endian.EndianNat. spec%param%0!))
      (vstd!endian.EndianNat./EndianNat/endian (%Poly%vstd!endian.EndianNat. spec%param%1!))
   )))
   :pattern ((req%vstd!endian.impl&%0.append. B&. B& spec%param%0! spec%param%1!))
   :qid internal_req__vstd!endian.impl&__0.append._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.append._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.append.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.append.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
    (= (vstd!endian.impl&%0.append.? B&. B& spec%param%0! spec%param%1!) (vstd!endian.impl&%0.new.?
      B&. B& (Poly%vstd!endian.Endian. (vstd!endian.EndianNat./EndianNat/endian (%Poly%vstd!endian.EndianNat.
         spec%param%0!
       ))
      ) (vstd!seq.Seq.add.? $ INT (Poly%vstd!seq.Seq<int.>. (vstd!endian.EndianNat./EndianNat/digits
         (%Poly%vstd!endian.EndianNat. spec%param%0!)
        )
       ) (Poly%vstd!seq.Seq<int.>. (vstd!endian.EndianNat./EndianNat/digits (%Poly%vstd!endian.EndianNat.
          spec%param%1!
    ))))))
    :pattern ((vstd!endian.impl&%0.append.? B&. B& spec%param%0! spec%param%1!))
    :qid internal_vstd!endian.impl&__0.append.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.append.?_definition
))))
(declare-fun req%vstd!endian.impl&%0.append_least. (Dcr Type Poly Poly) Bool)
(declare-const %%global_location_label%%31 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (= (req%vstd!endian.impl&%0.append_least. B&. B& spec%param%0! spec%param%1!) (=>
     %%global_location_label%%31
     (= (vstd!endian.EndianNat./EndianNat/endian (%Poly%vstd!endian.EndianNat. spec%param%0!))
      (vstd!endian.EndianNat./EndianNat/endian (%Poly%vstd!endian.EndianNat. spec%param%1!))
   )))
   :pattern ((req%vstd!endian.impl&%0.append_least. B&. B& spec%param%0! spec%param%1!))
   :qid internal_req__vstd!endian.impl&__0.append_least._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.append_least._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.append_least.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.append_least.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
    (= (vstd!endian.impl&%0.append_least.? B&. B& spec%param%0! spec%param%1!) (ite
      (is-vstd!endian.Endian./Little (vstd!endian.EndianNat./EndianNat/endian (%Poly%vstd!endian.EndianNat.
         spec%param%0!
      )))
      (vstd!endian.impl&%0.append.? B&. B& spec%param%1! spec%param%0!)
      (vstd!endian.impl&%0.append.? B&. B& spec%param%0! spec%param%1!)
    ))
    :pattern ((vstd!endian.impl&%0.append_least.? B&. B& spec%param%0! spec%param%1!))
    :qid internal_vstd!endian.impl&__0.append_least.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.append_least.?_definition
))))
(declare-fun req%vstd!endian.impl&%0.append_most. (Dcr Type Poly Poly) Bool)
(declare-const %%global_location_label%%32 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (= (req%vstd!endian.impl&%0.append_most. B&. B& spec%param%0! spec%param%1!) (=>
     %%global_location_label%%32
     (= (vstd!endian.EndianNat./EndianNat/endian (%Poly%vstd!endian.EndianNat. spec%param%0!))
      (vstd!endian.EndianNat./EndianNat/endian (%Poly%vstd!endian.EndianNat. spec%param%1!))
   )))
   :pattern ((req%vstd!endian.impl&%0.append_most. B&. B& spec%param%0! spec%param%1!))
   :qid internal_req__vstd!endian.impl&__0.append_most._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.append_most._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.append_most.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.append_most.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
    (= (vstd!endian.impl&%0.append_most.? B&. B& spec%param%0! spec%param%1!) (ite
      (is-vstd!endian.Endian./Little (vstd!endian.EndianNat./EndianNat/endian (%Poly%vstd!endian.EndianNat.
         spec%param%0!
      )))
      (vstd!endian.impl&%0.append.? B&. B& spec%param%0! spec%param%1!)
      (vstd!endian.impl&%0.append.? B&. B& spec%param%1! spec%param%0!)
    ))
    :pattern ((vstd!endian.impl&%0.append_most.? B&. B& spec%param%0! spec%param%1!))
    :qid internal_vstd!endian.impl&__0.append_most.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.append_most.?_definition
))))
(declare-fun req%vstd!endian.impl&%0.push_first. (Dcr Type Poly Poly) Bool)
(declare-const %%global_location_label%%33 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (= (req%vstd!endian.impl&%0.push_first. B&. B& spec%param%0! spec%param%1!) (=>
     %%global_location_label%%33
     (< (%I spec%param%1!) (%I (vstd!endian.Base.base.? B&. B&)))
   ))
   :pattern ((req%vstd!endian.impl&%0.push_first. B&. B& spec%param%0! spec%param%1!))
   :qid internal_req__vstd!endian.impl&__0.push_first._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.push_first._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.push_first.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.push_first.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
    (= (vstd!endian.impl&%0.push_first.? B&. B& spec%param%0! spec%param%1!) (vstd!endian.EndianNat./EndianNat
      (%Poly%vstd!endian.Endian. (Poly%vstd!endian.Endian. (vstd!endian.EndianNat./EndianNat/endian
         (%Poly%vstd!endian.EndianNat. spec%param%0!)
       ))
      ) (%Poly%vstd!seq.Seq<int.>. (vstd!seq.Seq.add.? $ INT (vstd!seq.Seq.push.? $ INT (vstd!seq.Seq.empty.?
          $ INT
         ) spec%param%1!
        ) (Poly%vstd!seq.Seq<int.>. (vstd!endian.EndianNat./EndianNat/digits (%Poly%vstd!endian.EndianNat.
           spec%param%0!
       ))))
      ) (%Poly%core!marker.PhantomData. (Poly%core!marker.PhantomData. (vstd!endian.EndianNat./EndianNat/phantom
         (%Poly%vstd!endian.EndianNat. spec%param%0!)
    )))))
    :pattern ((vstd!endian.impl&%0.push_first.? B&. B& spec%param%0! spec%param%1!))
    :qid internal_vstd!endian.impl&__0.push_first.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.push_first.?_definition
))))
(declare-fun req%vstd!endian.impl&%0.push_last. (Dcr Type Poly Poly) Bool)
(declare-const %%global_location_label%%34 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (= (req%vstd!endian.impl&%0.push_last. B&. B& spec%param%0! spec%param%1!) (=>
     %%global_location_label%%34
     (< (%I spec%param%1!) (%I (vstd!endian.Base.base.? B&. B&)))
   ))
   :pattern ((req%vstd!endian.impl&%0.push_last. B&. B& spec%param%0! spec%param%1!))
   :qid internal_req__vstd!endian.impl&__0.push_last._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.push_last._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.push_last.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.push_last.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
    (= (vstd!endian.impl&%0.push_last.? B&. B& spec%param%0! spec%param%1!) (vstd!endian.EndianNat./EndianNat
      (%Poly%vstd!endian.Endian. (Poly%vstd!endian.Endian. (vstd!endian.EndianNat./EndianNat/endian
         (%Poly%vstd!endian.EndianNat. spec%param%0!)
       ))
      ) (%Poly%vstd!seq.Seq<int.>. (vstd!seq.Seq.push.? $ INT (Poly%vstd!seq.Seq<int.>. (vstd!endian.EndianNat./EndianNat/digits
          (%Poly%vstd!endian.EndianNat. spec%param%0!)
         )
        ) spec%param%1!
       )
      ) (%Poly%core!marker.PhantomData. (Poly%core!marker.PhantomData. (vstd!endian.EndianNat./EndianNat/phantom
         (%Poly%vstd!endian.EndianNat. spec%param%0!)
    )))))
    :pattern ((vstd!endian.impl&%0.push_last.? B&. B& spec%param%0! spec%param%1!))
    :qid internal_vstd!endian.impl&__0.push_last.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.push_last.?_definition
))))
(declare-fun req%vstd!endian.impl&%0.push_least. (Dcr Type Poly Poly) Bool)
(declare-const %%global_location_label%%35 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (= (req%vstd!endian.impl&%0.push_least. B&. B& spec%param%0! spec%param%1!) (=>
     %%global_location_label%%35
     (< (%I spec%param%1!) (%I (vstd!endian.Base.base.? B&. B&)))
   ))
   :pattern ((req%vstd!endian.impl&%0.push_least. B&. B& spec%param%0! spec%param%1!))
   :qid internal_req__vstd!endian.impl&__0.push_least._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.push_least._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.push_least.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.push_least.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
    (= (vstd!endian.impl&%0.push_least.? B&. B& spec%param%0! spec%param%1!) (ite
      (is-vstd!endian.Endian./Little (vstd!endian.EndianNat./EndianNat/endian (%Poly%vstd!endian.EndianNat.
         spec%param%0!
      )))
      (vstd!endian.impl&%0.push_first.? B&. B& spec%param%0! spec%param%1!)
      (vstd!endian.impl&%0.push_last.? B&. B& spec%param%0! spec%param%1!)
    ))
    :pattern ((vstd!endian.impl&%0.push_least.? B&. B& spec%param%0! spec%param%1!))
    :qid internal_vstd!endian.impl&__0.push_least.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.push_least.?_definition
))))
(declare-fun req%vstd!endian.impl&%0.push_most. (Dcr Type Poly Poly) Bool)
(declare-const %%global_location_label%%36 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (= (req%vstd!endian.impl&%0.push_most. B&. B& spec%param%0! spec%param%1!) (=>
     %%global_location_label%%36
     (< (%I spec%param%1!) (%I (vstd!endian.Base.base.? B&. B&)))
   ))
   :pattern ((req%vstd!endian.impl&%0.push_most. B&. B& spec%param%0! spec%param%1!))
   :qid internal_req__vstd!endian.impl&__0.push_most._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.push_most._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.push_most.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.push_most.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
    (= (vstd!endian.impl&%0.push_most.? B&. B& spec%param%0! spec%param%1!) (ite
      (is-vstd!endian.Endian./Little (vstd!endian.EndianNat./EndianNat/endian (%Poly%vstd!endian.EndianNat.
         spec%param%0!
      )))
      (vstd!endian.impl&%0.push_last.? B&. B& spec%param%0! spec%param%1!)
      (vstd!endian.impl&%0.push_first.? B&. B& spec%param%0! spec%param%1!)
    ))
    :pattern ((vstd!endian.impl&%0.push_most.? B&. B& spec%param%0! spec%param%1!))
    :qid internal_vstd!endian.impl&__0.push_most.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.push_most.?_definition
))))
(declare-const fuel_nat%vstd!endian.impl&%0.to_nat. Fuel)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (fuel% Fuel)) (!
   (= (vstd!endian.impl&%0.rec%to_nat.? B&. B& spec%param%0! fuel%) (vstd!endian.impl&%0.rec%to_nat.?
     B&. B& spec%param%0! zero
   ))
   :pattern ((vstd!endian.impl&%0.rec%to_nat.? B&. B& spec%param%0! fuel%))
   :qid internal_vstd!endian.impl&__0.to_nat._fuel_to_zero_definition
   :skolemid skolem_internal_vstd!endian.impl&__0.to_nat._fuel_to_zero_definition
)))
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (fuel% Fuel)) (!
   (=>
    (and
     (sized B&.)
     (tr_bound%vstd!endian.Base. B&. B&)
     (has_type spec%param%0! (TYPE%vstd!endian.EndianNat. B&. B&))
    )
    (= (vstd!endian.impl&%0.rec%to_nat.? B&. B& spec%param%0! (succ fuel%)) (ite
      (= (vstd!endian.impl&%0.len.? B&. B& spec%param%0!) 0)
      0
      (nClip (Add (nClip (Mul (vstd!endian.impl&%0.rec%to_nat.? B&. B& (Poly%vstd!endian.EndianNat.
            (vstd!endian.impl&%0.drop_least.? B&. B& spec%param%0!)
           ) fuel%
          ) (%I (vstd!endian.Base.base.? B&. B&))
         )
        ) (vstd!endian.impl&%0.least.? B&. B& spec%param%0!)
   )))))
   :pattern ((vstd!endian.impl&%0.rec%to_nat.? B&. B& spec%param%0! (succ fuel%)))
   :qid internal_vstd!endian.impl&__0.to_nat._fuel_to_body_definition
   :skolemid skolem_internal_vstd!endian.impl&__0.to_nat._fuel_to_body_definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.to_nat.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
    (=>
     (and
      (sized B&.)
      (tr_bound%vstd!endian.Base. B&. B&)
      (has_type spec%param%0! (TYPE%vstd!endian.EndianNat. B&. B&))
     )
     (= (vstd!endian.impl&%0.to_nat.? B&. B& spec%param%0!) (vstd!endian.impl&%0.rec%to_nat.?
       B&. B& spec%param%0! (succ fuel_nat%vstd!endian.impl&%0.to_nat.)
    )))
    :pattern ((vstd!endian.impl&%0.to_nat.? B&. B& spec%param%0!))
    :qid internal_vstd!endian.impl&__0.to_nat.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.to_nat.?_definition
))))
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
   (=>
    (has_type spec%param%0! (TYPE%vstd!endian.EndianNat. B&. B&))
    (<= 0 (vstd!endian.impl&%0.to_nat.? B&. B& spec%param%0!))
   )
   :pattern ((vstd!endian.impl&%0.to_nat.? B&. B& spec%param%0!))
   :qid internal_vstd!endian.impl&__0.to_nat.?_pre_post_definition
   :skolemid skolem_internal_vstd!endian.impl&__0.to_nat.?_pre_post_definition
)))
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (fuel% Fuel)) (!
   (=>
    (has_type spec%param%0! (TYPE%vstd!endian.EndianNat. B&. B&))
    (<= 0 (vstd!endian.impl&%0.rec%to_nat.? B&. B& spec%param%0! fuel%))
   )
   :pattern ((vstd!endian.impl&%0.rec%to_nat.? B&. B& spec%param%0! fuel%))
   :qid internal_vstd!endian.impl&__0.rec__to_nat.?_pre_post_rec_definition
   :skolemid skolem_internal_vstd!endian.impl&__0.rec__to_nat.?_pre_post_rec_definition
)))
(declare-fun req%vstd!endian.impl&%0.from_nat. (Dcr Type Poly Poly) Bool)
(declare-const %%global_location_label%%37 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
   (= (req%vstd!endian.impl&%0.from_nat. B&. B& spec%param%0! spec%param%1!) (=>
     %%global_location_label%%37
     (< (%I spec%param%0!) (vstd!endian.base_upper_bound_excl.? B&. B& spec%param%1!))
   ))
   :pattern ((req%vstd!endian.impl&%0.from_nat. B&. B& spec%param%0! spec%param%1!))
   :qid internal_req__vstd!endian.impl&__0.from_nat._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.from_nat._definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%0.from_nat.)
)
(declare-const fuel_nat%vstd!endian.impl&%0.from_nat. Fuel)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly) (fuel% Fuel))
  (!
   (= (vstd!endian.impl&%0.rec%from_nat.? B&. B& spec%param%0! spec%param%1! fuel%) (
     vstd!endian.impl&%0.rec%from_nat.? B&. B& spec%param%0! spec%param%1! zero
   ))
   :pattern ((vstd!endian.impl&%0.rec%from_nat.? B&. B& spec%param%0! spec%param%1! fuel%))
   :qid internal_vstd!endian.impl&__0.from_nat._fuel_to_zero_definition
   :skolemid skolem_internal_vstd!endian.impl&__0.from_nat._fuel_to_zero_definition
)))
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly) (fuel% Fuel))
  (!
   (=>
    (and
     (sized B&.)
     (tr_bound%vstd!endian.Base. B&. B&)
     (has_type spec%param%0! NAT)
     (has_type spec%param%1! NAT)
    )
    (= (vstd!endian.impl&%0.rec%from_nat.? B&. B& spec%param%0! spec%param%1! (succ fuel%))
     (ite
      (= (%I spec%param%1!) 0)
      (vstd!endian.EndianNat./EndianNat (%Poly%vstd!endian.Endian. (Poly%vstd!endian.Endian.
         (vstd!endian.endianness.? (I 0))
        )
       ) (%Poly%vstd!seq.Seq<int.>. (vstd!seq.Seq.empty.? $ INT)) (%Poly%core!marker.PhantomData.
        (Poly%core!marker.PhantomData. core!marker.PhantomData./PhantomData)
      ))
      (let
       ((least$ (EucMod (%I spec%param%0!) (%I (vstd!endian.Base.base.? B&. B&)))))
       (let
        ((rest$ (EucDiv (%I spec%param%0!) (%I (vstd!endian.Base.base.? B&. B&)))))
        (let
         ((rest_endian$ (vstd!endian.impl&%0.rec%from_nat.? B&. B& (I rest$) (I (nClip (Sub (%I spec%param%1!)
               1
             ))
            ) fuel%
         )))
         (vstd!endian.impl&%0.push_least.? B&. B& (Poly%vstd!endian.EndianNat. rest_endian$)
          (I least$)
   )))))))
   :pattern ((vstd!endian.impl&%0.rec%from_nat.? B&. B& spec%param%0! spec%param%1! (succ
      fuel%
   )))
   :qid internal_vstd!endian.impl&__0.from_nat._fuel_to_body_definition
   :skolemid skolem_internal_vstd!endian.impl&__0.from_nat._fuel_to_body_definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.from_nat.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (spec%param%1! Poly)) (!
    (=>
     (and
      (sized B&.)
      (tr_bound%vstd!endian.Base. B&. B&)
      (has_type spec%param%0! NAT)
      (has_type spec%param%1! NAT)
     )
     (= (vstd!endian.impl&%0.from_nat.? B&. B& spec%param%0! spec%param%1!) (vstd!endian.impl&%0.rec%from_nat.?
       B&. B& spec%param%0! spec%param%1! (succ fuel_nat%vstd!endian.impl&%0.from_nat.)
    )))
    :pattern ((vstd!endian.impl&%0.from_nat.? B&. B& spec%param%0! spec%param%1!))
    :qid internal_vstd!endian.impl&__0.from_nat.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.from_nat.?_definition
))))
(declare-const fuel_nat%vstd!endian.impl&%0.to_nat_most. Fuel)
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (fuel% Fuel)) (!
   (= (vstd!endian.impl&%0.rec%to_nat_most.? B&. B& spec%param%0! fuel%) (vstd!endian.impl&%0.rec%to_nat_most.?
     B&. B& spec%param%0! zero
   ))
   :pattern ((vstd!endian.impl&%0.rec%to_nat_most.? B&. B& spec%param%0! fuel%))
   :qid internal_vstd!endian.impl&__0.to_nat_most._fuel_to_zero_definition
   :skolemid skolem_internal_vstd!endian.impl&__0.to_nat_most._fuel_to_zero_definition
)))
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (fuel% Fuel)) (!
   (=>
    (and
     (sized B&.)
     (tr_bound%vstd!endian.Base. B&. B&)
     (has_type spec%param%0! (TYPE%vstd!endian.EndianNat. B&. B&))
    )
    (= (vstd!endian.impl&%0.rec%to_nat_most.? B&. B& spec%param%0! (succ fuel%)) (ite
      (= (vstd!endian.impl&%0.len.? B&. B& spec%param%0!) 0)
      0
      (nClip (Add (vstd!endian.impl&%0.rec%to_nat_most.? B&. B& (Poly%vstd!endian.EndianNat.
          (vstd!endian.impl&%0.drop_most.? B&. B& spec%param%0!)
         ) fuel%
        ) (Mul (vstd!endian.impl&%0.most.? B&. B& spec%param%0!) (vstd!arithmetic.power.pow.?
          (vstd!endian.Base.base.? B&. B&) (I (nClip (Sub (vstd!endian.impl&%0.len.? B&. B& spec%param%0!)
             1
   ))))))))))
   :pattern ((vstd!endian.impl&%0.rec%to_nat_most.? B&. B& spec%param%0! (succ fuel%)))
   :qid internal_vstd!endian.impl&__0.to_nat_most._fuel_to_body_definition
   :skolemid skolem_internal_vstd!endian.impl&__0.to_nat_most._fuel_to_body_definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.to_nat_most.)
  (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
    (=>
     (and
      (sized B&.)
      (tr_bound%vstd!endian.Base. B&. B&)
      (has_type spec%param%0! (TYPE%vstd!endian.EndianNat. B&. B&))
     )
     (= (vstd!endian.impl&%0.to_nat_most.? B&. B& spec%param%0!) (vstd!endian.impl&%0.rec%to_nat_most.?
       B&. B& spec%param%0! (succ fuel_nat%vstd!endian.impl&%0.to_nat_most.)
    )))
    :pattern ((vstd!endian.impl&%0.to_nat_most.? B&. B& spec%param%0!))
    :qid internal_vstd!endian.impl&__0.to_nat_most.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__0.to_nat_most.?_definition
))))
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
   (=>
    (has_type spec%param%0! (TYPE%vstd!endian.EndianNat. B&. B&))
    (<= 0 (vstd!endian.impl&%0.to_nat_most.? B&. B& spec%param%0!))
   )
   :pattern ((vstd!endian.impl&%0.to_nat_most.? B&. B& spec%param%0!))
   :qid internal_vstd!endian.impl&__0.to_nat_most.?_pre_post_definition
   :skolemid skolem_internal_vstd!endian.impl&__0.to_nat_most.?_pre_post_definition
)))
(assert
 (forall ((B&. Dcr) (B& Type) (spec%param%0! Poly) (fuel% Fuel)) (!
   (=>
    (has_type spec%param%0! (TYPE%vstd!endian.EndianNat. B&. B&))
    (<= 0 (vstd!endian.impl&%0.rec%to_nat_most.? B&. B& spec%param%0! fuel%))
   )
   :pattern ((vstd!endian.impl&%0.rec%to_nat_most.? B&. B& spec%param%0! fuel%))
   :qid internal_vstd!endian.impl&__0.rec__to_nat_most.?_pre_post_rec_definition
   :skolemid skolem_internal_vstd!endian.impl&__0.rec__to_nat_most.?_pre_post_rec_definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%1.exp.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%1.exp.)
  (forall ((B&. Dcr) (B& Type) (BIG&. Dcr) (BIG& Type)) (!
    (= (vstd!endian.impl&%1.exp.? B&. B& BIG&. BIG&) (EucDiv (%I (vstd!endian.BasePow2.bits.?
        BIG&. BIG&
       )
      ) (%I (vstd!endian.BasePow2.bits.? B&. B&))
    ))
    :pattern ((vstd!endian.impl&%1.exp.? B&. B& BIG&. BIG&))
    :qid internal_vstd!endian.impl&__1.exp.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__1.exp.?_definition
))))
(assert
 (forall ((B&. Dcr) (B& Type) (BIG&. Dcr) (BIG& Type)) (!
   (<= 0 (vstd!endian.impl&%1.exp.? B&. B& BIG&. BIG&))
   :pattern ((vstd!endian.impl&%1.exp.? B&. B& BIG&. BIG&))
   :qid internal_vstd!endian.impl&__1.exp.?_pre_post_definition
   :skolemid skolem_internal_vstd!endian.impl&__1.exp.?_pre_post_definition
)))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%1.from_big.)
)
(declare-const fuel_nat%vstd!endian.impl&%1.from_big. Fuel)
(assert
 (forall ((B&. Dcr) (B& Type) (BIG&. Dcr) (BIG& Type) (spec%param%0! Poly) (fuel% Fuel))
  (!
   (= (vstd!endian.impl&%1.rec%from_big.? B&. B& BIG&. BIG& spec%param%0! fuel%) (vstd!endian.impl&%1.rec%from_big.?
     B&. B& BIG&. BIG& spec%param%0! zero
   ))
   :pattern ((vstd!endian.impl&%1.rec%from_big.? B&. B& BIG&. BIG& spec%param%0! fuel%))
   :qid internal_vstd!endian.impl&__1.from_big._fuel_to_zero_definition
   :skolemid skolem_internal_vstd!endian.impl&__1.from_big._fuel_to_zero_definition
)))
(assert
 (forall ((B&. Dcr) (B& Type) (BIG&. Dcr) (BIG& Type) (spec%param%0! Poly) (fuel% Fuel))
  (!
   (=>
    (and
     (sized B&.)
     (tr_bound%vstd!endian.Base. B&. B&)
     (sized BIG&.)
     (tr_bound%vstd!endian.BasePow2. BIG&. BIG&)
     (tr_bound%vstd!endian.CompatibleSmallerBaseFor. B&. B& BIG&. BIG&)
     (has_type spec%param%0! (TYPE%vstd!endian.EndianNat. BIG&. BIG&))
    )
    (= (vstd!endian.impl&%1.rec%from_big.? B&. B& BIG&. BIG& spec%param%0! (succ fuel%))
     (ite
      (= (vstd!endian.impl&%0.len.? BIG&. BIG& spec%param%0!) 0)
      (vstd!endian.impl&%0.new.? B&. B& (Poly%vstd!endian.Endian. (vstd!endian.EndianNat./EndianNat/endian
         (%Poly%vstd!endian.EndianNat. spec%param%0!)
        )
       ) (vstd!seq.Seq.empty.? $ INT)
      )
      (vstd!endian.impl&%0.append_least.? B&. B& (Poly%vstd!endian.EndianNat. (vstd!endian.impl&%1.rec%from_big.?
         B&. B& BIG&. BIG& (Poly%vstd!endian.EndianNat. (vstd!endian.impl&%0.drop_least.? BIG&.
           BIG& spec%param%0!
          )
         ) fuel%
        )
       ) (Poly%vstd!endian.EndianNat. (vstd!endian.impl&%0.from_nat.? B&. B& (I (vstd!endian.impl&%0.least.?
           BIG&. BIG& spec%param%0!
          )
         ) (I (vstd!endian.impl&%1.exp.? B&. B& BIG&. BIG&))
   ))))))
   :pattern ((vstd!endian.impl&%1.rec%from_big.? B&. B& BIG&. BIG& spec%param%0! (succ
      fuel%
   )))
   :qid internal_vstd!endian.impl&__1.from_big._fuel_to_body_definition
   :skolemid skolem_internal_vstd!endian.impl&__1.from_big._fuel_to_body_definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%1.from_big.)
  (forall ((B&. Dcr) (B& Type) (BIG&. Dcr) (BIG& Type) (spec%param%0! Poly)) (!
    (=>
     (and
      (sized B&.)
      (tr_bound%vstd!endian.Base. B&. B&)
      (sized BIG&.)
      (tr_bound%vstd!endian.BasePow2. BIG&. BIG&)
      (tr_bound%vstd!endian.CompatibleSmallerBaseFor. B&. B& BIG&. BIG&)
      (has_type spec%param%0! (TYPE%vstd!endian.EndianNat. BIG&. BIG&))
     )
     (= (vstd!endian.impl&%1.from_big.? B&. B& BIG&. BIG& spec%param%0!) (vstd!endian.impl&%1.rec%from_big.?
       B&. B& BIG&. BIG& spec%param%0! (succ fuel_nat%vstd!endian.impl&%1.from_big.)
    )))
    :pattern ((vstd!endian.impl&%1.from_big.? B&. B& BIG&. BIG& spec%param%0!))
    :qid internal_vstd!endian.impl&__1.from_big.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__1.from_big.?_definition
))))
(assert
 (=>
  (fuel_bool fuel%vstd!arithmetic.div_mod.lemma_div_basics_1.)
  (forall ((x! Int)) (!
    (=>
     (not (= x! 0))
     (= (EucDiv 0 x!) 0)
    )
    :pattern ((EucDiv 0 x!))
    :qid user_vstd__arithmetic__div_mod__lemma_div_basics_1_0
    :skolemid skolem_user_vstd__arithmetic__div_mod__lemma_div_basics_1_0
))))
(assert
 (=>
  (fuel_bool fuel%vstd!arithmetic.div_mod.lemma_div_basics_2.)
  (forall ((x! Int)) (!
    (= (EucDiv x! 1) x!)
    :pattern ((EucDiv x! 1))
    :qid user_vstd__arithmetic__div_mod__lemma_div_basics_2_0
    :skolemid skolem_user_vstd__arithmetic__div_mod__lemma_div_basics_2_0
))))
(assert
 (=>
  (fuel_bool fuel%vstd!arithmetic.div_mod.lemma_div_basics_3.)
  (forall ((x! Int)) (!
    (=>
     (not (= x! 0))
     (= (EucDiv x! x!) 1)
    )
    :pattern ((EucDiv x! x!))
    :qid user_vstd__arithmetic__div_mod__lemma_div_basics_3_0
    :skolemid skolem_user_vstd__arithmetic__div_mod__lemma_div_basics_3_0
))))
(assert
 (=>
  (fuel_bool fuel%vstd!arithmetic.div_mod.lemma_div_basics_4.)
  (forall ((x! Int) (y! Int)) (!
    (=>
     (and
      (>= x! 0)
      (> y! 0)
     )
     (>= (EucDiv x! y!) 0)
    )
    :pattern ((EucDiv x! y!))
    :qid user_vstd__arithmetic__div_mod__lemma_div_basics_4_0
    :skolemid skolem_user_vstd__arithmetic__div_mod__lemma_div_basics_4_0
))))
(declare-fun req%vstd!arithmetic.mul.lemma_mul_inequality. (Int Int Int) Bool)
(declare-const %%global_location_label%%38 Bool)
(declare-const %%global_location_label%%39 Bool)
(assert
 (forall ((x! Int) (y! Int) (z! Int)) (!
   (= (req%vstd!arithmetic.mul.lemma_mul_inequality. x! y! z!) (and
     (=>
      %%global_location_label%%38
      (<= x! y!)
     )
     (=>
      %%global_location_label%%39
      (>= z! 0)
   )))
   :pattern ((req%vstd!arithmetic.mul.lemma_mul_inequality. x! y! z!))
   :qid internal_req__vstd!arithmetic.mul.lemma_mul_inequality._definition
   :skolemid skolem_internal_req__vstd!arithmetic.mul.lemma_mul_inequality._definition
)))
(declare-fun ens%vstd!arithmetic.mul.lemma_mul_inequality. (Int Int Int) Bool)
(assert
 (forall ((x! Int) (y! Int) (z! Int)) (!
   (= (ens%vstd!arithmetic.mul.lemma_mul_inequality. x! y! z!) (<= (Mul x! z!) (Mul y!
      z!
   )))
   :pattern ((ens%vstd!arithmetic.mul.lemma_mul_inequality. x! y! z!))
   :qid internal_ens__vstd!arithmetic.mul.lemma_mul_inequality._definition
   :skolemid skolem_internal_ens__vstd!arithmetic.mul.lemma_mul_inequality._definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!arithmetic.mul.lemma_mul_inequality.)
  (forall ((x! Int) (y! Int) (z! Int)) (!
    (=>
     (and
      (<= x! y!)
      (>= z! 0)
     )
     (<= (Mul x! z!) (Mul y! z!))
    )
    :pattern ((Mul x! z!) (Mul y! z!))
    :qid user_vstd__arithmetic__mul__lemma_mul_inequality_0
    :skolemid skolem_user_vstd__arithmetic__mul__lemma_mul_inequality_0
))))
(declare-fun ens%vstd!arithmetic.mul.lemma_mul_basics. (Int) Bool)
(assert
 (forall ((x! Int)) (!
   (= (ens%vstd!arithmetic.mul.lemma_mul_basics. x!) (and
     (= (Mul 0 x!) 0)
     (= (Mul x! 0) 0)
     (= (Mul x! 1) x!)
     (= (Mul 1 x!) x!)
   ))
   :pattern ((ens%vstd!arithmetic.mul.lemma_mul_basics. x!))
   :qid internal_ens__vstd!arithmetic.mul.lemma_mul_basics._definition
   :skolemid skolem_internal_ens__vstd!arithmetic.mul.lemma_mul_basics._definition
)))
(declare-fun ens%vstd!arithmetic.mul.lemma_mul_is_distributive_add_other_way. (Int
  Int Int
 ) Bool
)
(assert
 (forall ((x! Int) (y! Int) (z! Int)) (!
   (= (ens%vstd!arithmetic.mul.lemma_mul_is_distributive_add_other_way. x! y! z!) (= (
      Mul (Add y! z!) x!
     ) (Add (Mul y! x!) (Mul z! x!))
   ))
   :pattern ((ens%vstd!arithmetic.mul.lemma_mul_is_distributive_add_other_way. x! y! z!))
   :qid internal_ens__vstd!arithmetic.mul.lemma_mul_is_distributive_add_other_way._definition
   :skolemid skolem_internal_ens__vstd!arithmetic.mul.lemma_mul_is_distributive_add_other_way._definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!arithmetic.mul.lemma_mul_is_distributive_add_other_way.)
  (forall ((x! Int) (y! Int) (z! Int)) (!
    (= (Mul (Add y! z!) x!) (Add (Mul y! x!) (Mul z! x!)))
    :pattern ((Mul (Add y! z!) x!))
    :qid user_vstd__arithmetic__mul__lemma_mul_is_distributive_add_other_way_0
    :skolemid skolem_user_vstd__arithmetic__mul__lemma_mul_is_distributive_add_other_way_0
))))
(declare-fun ens%vstd!arithmetic.mul.lemma_mul_is_distributive_sub_other_way. (Int
  Int Int
 ) Bool
)
(assert
 (forall ((x! Int) (y! Int) (z! Int)) (!
   (= (ens%vstd!arithmetic.mul.lemma_mul_is_distributive_sub_other_way. x! y! z!) (= (
      Mul (Sub y! z!) x!
     ) (Sub (Mul y! x!) (Mul z! x!))
   ))
   :pattern ((ens%vstd!arithmetic.mul.lemma_mul_is_distributive_sub_other_way. x! y! z!))
   :qid internal_ens__vstd!arithmetic.mul.lemma_mul_is_distributive_sub_other_way._definition
   :skolemid skolem_internal_ens__vstd!arithmetic.mul.lemma_mul_is_distributive_sub_other_way._definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!arithmetic.mul.lemma_mul_is_distributive_sub_other_way.)
  (forall ((x! Int) (y! Int) (z! Int)) (!
    (= (Mul (Sub y! z!) x!) (Sub (Mul y! x!) (Mul z! x!)))
    :pattern ((Mul (Sub y! z!) x!))
    :qid user_vstd__arithmetic__mul__lemma_mul_is_distributive_sub_other_way_0
    :skolemid skolem_user_vstd__arithmetic__mul__lemma_mul_is_distributive_sub_other_way_0
))))
(assert
 (=>
  (fuel_bool fuel%vstd!arithmetic.div_mod.lemma_div_basics_5.)
  (forall ((x! Int) (y! Int)) (!
    (=>
     (and
      (>= x! 0)
      (> y! 0)
     )
     (<= (EucDiv x! y!) x!)
    )
    :pattern ((EucDiv x! y!))
    :qid user_vstd__arithmetic__div_mod__lemma_div_basics_5_0
    :skolemid skolem_user_vstd__arithmetic__div_mod__lemma_div_basics_5_0
))))
(declare-fun ens%vstd!arithmetic.mul.lemma_mul_is_associative. (Int Int Int) Bool)
(assert
 (forall ((x! Int) (y! Int) (z! Int)) (!
   (= (ens%vstd!arithmetic.mul.lemma_mul_is_associative. x! y! z!) (= (Mul x! (Mul y! z!))
     (Mul (Mul x! y!) z!)
   ))
   :pattern ((ens%vstd!arithmetic.mul.lemma_mul_is_associative. x! y! z!))
   :qid internal_ens__vstd!arithmetic.mul.lemma_mul_is_associative._definition
   :skolemid skolem_internal_ens__vstd!arithmetic.mul.lemma_mul_is_associative._definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!arithmetic.mul.lemma_mul_is_associative.)
  (forall ((x! Int) (y! Int) (z! Int)) (!
    (= (Mul x! (Mul y! z!)) (Mul (Mul x! y!) z!))
    :pattern ((Mul x! (Mul y! z!)))
    :pattern ((Mul (Mul x! y!) z!))
    :qid user_vstd__arithmetic__mul__lemma_mul_is_associative_0
    :skolemid skolem_user_vstd__arithmetic__mul__lemma_mul_is_associative_0
))))
(declare-fun ens%vstd!arithmetic.power.lemma_pow_adds. (Int Int Int) Bool)
(assert
 (forall ((b! Int) (e1! Int) (e2! Int)) (!
   (= (ens%vstd!arithmetic.power.lemma_pow_adds. b! e1! e2!) (= (vstd!arithmetic.power.pow.?
      (I b!) (I (nClip (Add e1! e2!)))
     ) (Mul (vstd!arithmetic.power.pow.? (I b!) (I e1!)) (vstd!arithmetic.power.pow.? (I
        b!
       ) (I e2!)
   ))))
   :pattern ((ens%vstd!arithmetic.power.lemma_pow_adds. b! e1! e2!))
   :qid internal_ens__vstd!arithmetic.power.lemma_pow_adds._definition
   :skolemid skolem_internal_ens__vstd!arithmetic.power.lemma_pow_adds._definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!arithmetic.power.lemma_pow_adds.)
  (forall ((b! Poly) (e1! Int) (e2! Int)) (!
    (=>
     (and
      (has_type b! INT)
      (<= 0 e1!)
      (<= 0 e2!)
     )
     (= (vstd!arithmetic.power.pow.? b! (I (nClip (Add e1! e2!)))) (Mul (vstd!arithmetic.power.pow.?
        b! (I e1!)
       ) (vstd!arithmetic.power.pow.? b! (I e2!))
    )))
    :pattern ((vstd!arithmetic.power.pow.? b! (I (nClip (Add e1! e2!)))))
    :qid user_vstd__arithmetic__power__lemma_pow_adds_0
    :skolemid skolem_user_vstd__arithmetic__power__lemma_pow_adds_0
))))
(declare-fun ens%vstd!arithmetic.power.lemma_pow_multiplies. (Int Int Int) Bool)
(assert
 (forall ((a! Int) (b! Int) (c! Int)) (!
   (= (ens%vstd!arithmetic.power.lemma_pow_multiplies. a! b! c!) (and
     (<= 0 (nClip (Mul b! c!)))
     (= (vstd!arithmetic.power.pow.? (I (vstd!arithmetic.power.pow.? (I a!) (I b!))) (I c!))
      (vstd!arithmetic.power.pow.? (I a!) (I (nClip (Mul b! c!))))
   )))
   :pattern ((ens%vstd!arithmetic.power.lemma_pow_multiplies. a! b! c!))
   :qid internal_ens__vstd!arithmetic.power.lemma_pow_multiplies._definition
   :skolemid skolem_internal_ens__vstd!arithmetic.power.lemma_pow_multiplies._definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!arithmetic.power.lemma_pow_multiplies.)
  (forall ((a! Poly) (b! Poly) (c! Poly)) (!
    (=>
     (and
      (has_type a! INT)
      (has_type b! NAT)
      (has_type c! NAT)
     )
     (and
      (<= 0 (nClip (Mul (%I b!) (%I c!))))
      (= (vstd!arithmetic.power.pow.? (I (vstd!arithmetic.power.pow.? a! b!)) c!) (vstd!arithmetic.power.pow.?
        a! (I (nClip (Mul (%I b!) (%I c!))))
    ))))
    :pattern ((vstd!arithmetic.power.pow.? (I (vstd!arithmetic.power.pow.? a! b!)) c!))
    :qid user_vstd__arithmetic__power__lemma_pow_multiplies_0
    :skolemid skolem_user_vstd__arithmetic__power__lemma_pow_multiplies_0
))))
(declare-fun req%vstd!arithmetic.div_mod.lemma_fundamental_div_mod. (Int Int) Bool)
(declare-const %%global_location_label%%40 Bool)
(assert
 (forall ((x! Int) (d! Int)) (!
   (= (req%vstd!arithmetic.div_mod.lemma_fundamental_div_mod. x! d!) (=>
     %%global_location_label%%40
     (not (= d! 0))
   ))
   :pattern ((req%vstd!arithmetic.div_mod.lemma_fundamental_div_mod. x! d!))
   :qid internal_req__vstd!arithmetic.div_mod.lemma_fundamental_div_mod._definition
   :skolemid skolem_internal_req__vstd!arithmetic.div_mod.lemma_fundamental_div_mod._definition
)))
(declare-fun ens%vstd!arithmetic.div_mod.lemma_fundamental_div_mod. (Int Int) Bool)
(assert
 (forall ((x! Int) (d! Int)) (!
   (= (ens%vstd!arithmetic.div_mod.lemma_fundamental_div_mod. x! d!) (= x! (Add (Mul d!
       (EucDiv x! d!)
      ) (EucMod x! d!)
   )))
   :pattern ((ens%vstd!arithmetic.div_mod.lemma_fundamental_div_mod. x! d!))
   :qid internal_ens__vstd!arithmetic.div_mod.lemma_fundamental_div_mod._definition
   :skolemid skolem_internal_ens__vstd!arithmetic.div_mod.lemma_fundamental_div_mod._definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!arithmetic.div_mod.lemma_fundamental_div_mod.)
  (forall ((x! Int) (d! Int)) (!
    (=>
     (not (= d! 0))
     (= x! (Add (Mul d! (EucDiv x! d!)) (EucMod x! d!)))
    )
    :pattern ((Add (Mul d! (EucDiv x! d!)) (EucMod x! d!)))
    :qid user_vstd__arithmetic__div_mod__lemma_fundamental_div_mod_0
    :skolemid skolem_user_vstd__arithmetic__div_mod__lemma_fundamental_div_mod_0
))))
(declare-fun ens%vstd!arithmetic.power2.lemma_pow2. (Int) Bool)
(assert
 (forall ((e! Int)) (!
   (= (ens%vstd!arithmetic.power2.lemma_pow2. e!) (= (vstd!arithmetic.power2.pow2.? (I e!))
     (vstd!arithmetic.power.pow.? (I 2) (I e!))
   ))
   :pattern ((ens%vstd!arithmetic.power2.lemma_pow2. e!))
   :qid internal_ens__vstd!arithmetic.power2.lemma_pow2._definition
   :skolemid skolem_internal_ens__vstd!arithmetic.power2.lemma_pow2._definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!arithmetic.power2.lemma_pow2.)
  (forall ((e! Poly)) (!
    (=>
     (has_type e! NAT)
     (= (vstd!arithmetic.power2.pow2.? e!) (vstd!arithmetic.power.pow.? (I 2) e!))
    )
    :pattern ((vstd!arithmetic.power2.pow2.? e!))
    :qid user_vstd__arithmetic__power2__lemma_pow2_0
    :skolemid skolem_user_vstd__arithmetic__power2__lemma_pow2_0
))))
(declare-fun ens%vstd!endian.impl&%1.exp_properties. (Dcr Type Dcr Type) Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (BIG&. Dcr) (BIG& Type)) (!
   (= (ens%vstd!endian.impl&%1.exp_properties. B&. B& BIG&. BIG&) (and
     (= (vstd!endian.base_upper_bound_excl.? B&. B& (I (vstd!endian.impl&%1.exp.? B&. B& BIG&.
         BIG&
       ))
      ) (%I (vstd!endian.Base.base.? BIG&. BIG&))
     )
     (> (vstd!endian.impl&%1.exp.? B&. B& BIG&. BIG&) 0)
   ))
   :pattern ((ens%vstd!endian.impl&%1.exp_properties. B&. B& BIG&. BIG&))
   :qid internal_ens__vstd!endian.impl&__1.exp_properties._definition
   :skolemid skolem_internal_ens__vstd!endian.impl&__1.exp_properties._definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%1.exp_properties.)
  (forall ((B&. Dcr) (B& Type) (BIG&. Dcr) (BIG& Type)) (!
    (=>
     (and
      (and
       (and
        (and
         (sized B&.)
         (tr_bound%vstd!endian.Base. B&. B&)
        )
        (sized BIG&.)
       )
       (tr_bound%vstd!endian.BasePow2. BIG&. BIG&)
      )
      (tr_bound%vstd!endian.CompatibleSmallerBaseFor. B&. B& BIG&. BIG&)
     )
     (and
      (= (vstd!endian.base_upper_bound_excl.? B&. B& (I (vstd!endian.impl&%1.exp.? B&. B& BIG&.
          BIG&
        ))
       ) (%I (vstd!endian.Base.base.? BIG&. BIG&))
      )
      (> (vstd!endian.impl&%1.exp.? B&. B& BIG&. BIG&) 0)
    ))
    :pattern ((vstd!endian.impl&%1.exp.? B&. B& BIG&. BIG&))
    :qid user_vstd__endian__EndianNat__exp_properties_0
    :skolemid skolem_user_vstd__endian__EndianNat__exp_properties_0
))))
(declare-fun req%vstd!arithmetic.div_mod.lemma_small_mod. (Int Int) Bool)
(declare-const %%global_location_label%%41 Bool)
(declare-const %%global_location_label%%42 Bool)
(assert
 (forall ((x! Int) (m! Int)) (!
   (= (req%vstd!arithmetic.div_mod.lemma_small_mod. x! m!) (and
     (=>
      %%global_location_label%%41
      (< x! m!)
     )
     (=>
      %%global_location_label%%42
      (< 0 m!)
   )))
   :pattern ((req%vstd!arithmetic.div_mod.lemma_small_mod. x! m!))
   :qid internal_req__vstd!arithmetic.div_mod.lemma_small_mod._definition
   :skolemid skolem_internal_req__vstd!arithmetic.div_mod.lemma_small_mod._definition
)))
(declare-fun ens%vstd!arithmetic.div_mod.lemma_small_mod. (Int Int) Bool)
(assert
 (forall ((x! Int) (m! Int)) (!
   (= (ens%vstd!arithmetic.div_mod.lemma_small_mod. x! m!) (= (EucMod x! m!) x!))
   :pattern ((ens%vstd!arithmetic.div_mod.lemma_small_mod. x! m!))
   :qid internal_ens__vstd!arithmetic.div_mod.lemma_small_mod._definition
   :skolemid skolem_internal_ens__vstd!arithmetic.div_mod.lemma_small_mod._definition
)))
(declare-fun req%vstd!arithmetic.div_mod.lemma_mod_is_zero. (Int Int) Bool)
(declare-const %%global_location_label%%43 Bool)
(declare-const %%global_location_label%%44 Bool)
(assert
 (forall ((x! Int) (m! Int)) (!
   (= (req%vstd!arithmetic.div_mod.lemma_mod_is_zero. x! m!) (and
     (=>
      %%global_location_label%%43
      (and
       (> x! 0)
       (> m! 0)
     ))
     (=>
      %%global_location_label%%44
      (= (EucMod x! m!) 0)
   )))
   :pattern ((req%vstd!arithmetic.div_mod.lemma_mod_is_zero. x! m!))
   :qid internal_req__vstd!arithmetic.div_mod.lemma_mod_is_zero._definition
   :skolemid skolem_internal_req__vstd!arithmetic.div_mod.lemma_mod_is_zero._definition
)))
(declare-fun ens%vstd!arithmetic.div_mod.lemma_mod_is_zero. (Int Int) Bool)
(assert
 (forall ((x! Int) (m! Int)) (!
   (= (ens%vstd!arithmetic.div_mod.lemma_mod_is_zero. x! m!) (>= x! m!))
   :pattern ((ens%vstd!arithmetic.div_mod.lemma_mod_is_zero. x! m!))
   :qid internal_ens__vstd!arithmetic.div_mod.lemma_mod_is_zero._definition
   :skolemid skolem_internal_ens__vstd!arithmetic.div_mod.lemma_mod_is_zero._definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!arithmetic.div_mod.lemma_mod_is_zero.)
  (forall ((x! Int) (m! Int)) (!
    (=>
     (and
      (<= 0 x!)
      (<= 0 m!)
     )
     (=>
      (and
       (and
        (> x! 0)
        (> m! 0)
       )
       (= (EucMod x! m!) 0)
      )
      (>= x! m!)
    ))
    :pattern ((EucMod x! m!))
    :qid user_vstd__arithmetic__div_mod__lemma_mod_is_zero_0
    :skolemid skolem_user_vstd__arithmetic__div_mod__lemma_mod_is_zero_0
))))
(declare-fun req%vstd!endian.impl&%1.to_big. (Dcr Type Dcr Type Poly) Bool)
(declare-const %%global_location_label%%45 Bool)
(declare-const %%global_location_label%%46 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (BIG&. Dcr) (BIG& Type) (spec%param%0! Poly)) (!
   (= (req%vstd!endian.impl&%1.to_big. B&. B& BIG&. BIG& spec%param%0!) (and
     (=>
      %%global_location_label%%45
      (= (EucMod (vstd!endian.impl&%0.len.? B&. B& spec%param%0!) (vstd!endian.impl&%1.exp.?
         B&. B& BIG&. BIG&
        )
       ) 0
     ))
     (=>
      %%global_location_label%%46
      (= (EucMod (vstd!endian.impl&%0.len.? B&. B& spec%param%0!) (vstd!endian.impl&%1.exp.?
         B&. B& BIG&. BIG&
        )
       ) 0
   ))))
   :pattern ((req%vstd!endian.impl&%1.to_big. B&. B& BIG&. BIG& spec%param%0!))
   :qid internal_req__vstd!endian.impl&__1.to_big._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__1.to_big._definition
)))
(declare-const fuel_nat%vstd!endian.impl&%1.to_big. Fuel)
(assert
 (forall ((B&. Dcr) (B& Type) (BIG&. Dcr) (BIG& Type) (spec%param%0! Poly) (fuel% Fuel))
  (!
   (= (vstd!endian.impl&%1.rec%to_big.? B&. B& BIG&. BIG& spec%param%0! fuel%) (vstd!endian.impl&%1.rec%to_big.?
     B&. B& BIG&. BIG& spec%param%0! zero
   ))
   :pattern ((vstd!endian.impl&%1.rec%to_big.? B&. B& BIG&. BIG& spec%param%0! fuel%))
   :qid internal_vstd!endian.impl&__1.to_big._fuel_to_zero_definition
   :skolemid skolem_internal_vstd!endian.impl&__1.to_big._fuel_to_zero_definition
)))
(assert
 (forall ((B&. Dcr) (B& Type) (BIG&. Dcr) (BIG& Type) (spec%param%0! Poly) (fuel% Fuel))
  (!
   (=>
    (and
     (sized B&.)
     (tr_bound%vstd!endian.Base. B&. B&)
     (sized BIG&.)
     (tr_bound%vstd!endian.BasePow2. BIG&. BIG&)
     (tr_bound%vstd!endian.CompatibleSmallerBaseFor. B&. B& BIG&. BIG&)
     (has_type spec%param%0! (TYPE%vstd!endian.EndianNat. B&. B&))
     (= (EucMod (vstd!endian.impl&%0.len.? B&. B& spec%param%0!) (vstd!endian.impl&%1.exp.?
        B&. B& BIG&. BIG&
       )
      ) 0
    ))
    (= (vstd!endian.impl&%1.rec%to_big.? B&. B& BIG&. BIG& spec%param%0! (succ fuel%))
     (ite
      (= (vstd!endian.impl&%0.len.? B&. B& spec%param%0!) 0)
      (vstd!endian.impl&%0.new.? BIG&. BIG& (Poly%vstd!endian.Endian. (vstd!endian.EndianNat./EndianNat/endian
         (%Poly%vstd!endian.EndianNat. spec%param%0!)
        )
       ) (vstd!seq.Seq.empty.? $ INT)
      )
      (vstd!endian.impl&%0.append_least.? BIG&. BIG& (Poly%vstd!endian.EndianNat. (vstd!endian.impl&%1.rec%to_big.?
         B&. B& BIG&. BIG& (Poly%vstd!endian.EndianNat. (vstd!endian.impl&%0.skip_least.? B&.
           B& spec%param%0! (I (vstd!endian.impl&%1.exp.? B&. B& BIG&. BIG&))
          )
         ) fuel%
        )
       ) (Poly%vstd!endian.EndianNat. (vstd!endian.impl&%0.new.? BIG&. BIG& (Poly%vstd!endian.Endian.
          (vstd!endian.EndianNat./EndianNat/endian (%Poly%vstd!endian.EndianNat. spec%param%0!))
         ) (vstd!seq.Seq.push.? $ INT (vstd!seq.Seq.empty.? $ INT) (I (vstd!endian.impl&%0.to_nat.?
            B&. B& (Poly%vstd!endian.EndianNat. (vstd!endian.impl&%0.take_least.? B&. B& spec%param%0!
              (I (vstd!endian.impl&%1.exp.? B&. B& BIG&. BIG&))
   )))))))))))
   :pattern ((vstd!endian.impl&%1.rec%to_big.? B&. B& BIG&. BIG& spec%param%0! (succ fuel%)))
   :qid internal_vstd!endian.impl&__1.to_big._fuel_to_body_definition
   :skolemid skolem_internal_vstd!endian.impl&__1.to_big._fuel_to_body_definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%1.to_big.)
  (forall ((B&. Dcr) (B& Type) (BIG&. Dcr) (BIG& Type) (spec%param%0! Poly)) (!
    (=>
     (and
      (sized B&.)
      (tr_bound%vstd!endian.Base. B&. B&)
      (sized BIG&.)
      (tr_bound%vstd!endian.BasePow2. BIG&. BIG&)
      (tr_bound%vstd!endian.CompatibleSmallerBaseFor. B&. B& BIG&. BIG&)
      (has_type spec%param%0! (TYPE%vstd!endian.EndianNat. B&. B&))
      (= (EucMod (vstd!endian.impl&%0.len.? B&. B& spec%param%0!) (vstd!endian.impl&%1.exp.?
         B&. B& BIG&. BIG&
        )
       ) 0
     ))
     (= (vstd!endian.impl&%1.to_big.? B&. B& BIG&. BIG& spec%param%0!) (vstd!endian.impl&%1.rec%to_big.?
       B&. B& BIG&. BIG& spec%param%0! (succ fuel_nat%vstd!endian.impl&%1.to_big.)
    )))
    :pattern ((vstd!endian.impl&%1.to_big.? B&. B& BIG&. BIG& spec%param%0!))
    :qid internal_vstd!endian.impl&__1.to_big.?_definition
    :skolemid skolem_internal_vstd!endian.impl&__1.to_big.?_definition
))))
(assert
 (fuel_bool_default fuel%vstd!endian.to_big_from_digits.)
)
(declare-fun %%lambda%%1 (Type Type) %%Function%%)
(assert
 (forall ((%%x%% Type) (%%hole%%0 Type) (%%hole%%1 Type) (i$ Poly) (d$ Poly)) (!
   (=>
    (and
     (has_type i$ %%hole%%0)
     (has_type d$ %%hole%%1)
    )
    (= (%%apply%%1 (mk_fun %%x%% (%%lambda%%1 %%hole%%0 %%hole%%1)) i$ d$) d$)
   )
   :pattern ((%%apply%%1 (mk_fun %%x%% (%%lambda%%1 %%hole%%0 %%hole%%1)) i$ d$))
)))
(assert
 (=>
  (fuel_bool fuel%vstd!endian.to_big_from_digits.)
  (forall ((BIG&. Dcr) (BIG& Type) (B&. Dcr) (B& Type) (spec%param%0! Poly)) (!
    (= (vstd!endian.to_big_from_digits.? BIG&. BIG& B&. B& spec%param%0!) (vstd!endian.impl&%1.to_big.?
      B&. B& BIG&. BIG& (Poly%vstd!endian.EndianNat. (vstd!endian.impl&%0.new.? B&. B& (Poly%vstd!endian.Endian.
         (vstd!endian.endianness.? (I 0))
        ) (vstd!seq_lib.impl&%0.map.? B&. B& $ INT spec%param%0! (Poly%fun%2. (mk_fun (TYPE%fun%2.
            $ INT B&. B& $ INT
           ) (%%lambda%%1 INT B&)
    )))))))
    :pattern ((vstd!endian.to_big_from_digits.? BIG&. BIG& B&. B& spec%param%0!))
    :qid internal_vstd!endian.to_big_from_digits.?_definition
    :skolemid skolem_internal_vstd!endian.to_big_from_digits.?_definition
))))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%2.base.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%2.base.)
  (= (vstd!endian.Base.base.? $ (UINT 8)) (I (nClip (Add 255 1))))
))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%3.base.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%3.base.)
  (= (vstd!endian.Base.base.? $ (UINT 64)) (I (nClip (Add 18446744073709551615 1))))
))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%4.base.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%4.base.)
  (= (vstd!endian.Base.base.? $ USIZE) (I (nClip (Add (- (uHi SZ) 1) 1))))
))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%5.bits.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%5.bits.)
  (= (vstd!endian.BasePow2.bits.? $ (UINT 8)) (I 8))
))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%6.bits.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%6.bits.)
  (= (vstd!endian.BasePow2.bits.? $ (UINT 64)) (I 64))
))
(assert
 (fuel_bool_default fuel%vstd!endian.impl&%7.bits.)
)
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%7.bits.)
  (= (vstd!endian.BasePow2.bits.? $ USIZE) (I SZ))
))
(assert
 (tr_bound%vstd!endian.Base. $ (UINT 8))
)
(assert
 (tr_bound%vstd!endian.Base. $ (UINT 64))
)
(assert
 (tr_bound%vstd!endian.Base. $ USIZE)
)
(declare-fun ens%vstd!arithmetic.power2.lemma2_to64. (Int) Bool)
(assert
 (forall ((no%param Int)) (!
   (= (ens%vstd!arithmetic.power2.lemma2_to64. no%param) (and
     (= (vstd!arithmetic.power2.pow2.? (I 0)) 1)
     (= (vstd!arithmetic.power2.pow2.? (I 1)) 2)
     (= (vstd!arithmetic.power2.pow2.? (I 2)) 4)
     (= (vstd!arithmetic.power2.pow2.? (I 3)) 8)
     (= (vstd!arithmetic.power2.pow2.? (I 4)) 16)
     (= (vstd!arithmetic.power2.pow2.? (I 5)) 32)
     (= (vstd!arithmetic.power2.pow2.? (I 6)) 64)
     (= (vstd!arithmetic.power2.pow2.? (I 7)) 128)
     (= (vstd!arithmetic.power2.pow2.? (I 8)) 256)
     (= (vstd!arithmetic.power2.pow2.? (I 9)) 512)
     (= (vstd!arithmetic.power2.pow2.? (I 10)) 1024)
     (= (vstd!arithmetic.power2.pow2.? (I 11)) 2048)
     (= (vstd!arithmetic.power2.pow2.? (I 12)) 4096)
     (= (vstd!arithmetic.power2.pow2.? (I 13)) 8192)
     (= (vstd!arithmetic.power2.pow2.? (I 14)) 16384)
     (= (vstd!arithmetic.power2.pow2.? (I 15)) 32768)
     (= (vstd!arithmetic.power2.pow2.? (I 16)) 65536)
     (= (vstd!arithmetic.power2.pow2.? (I 17)) 131072)
     (= (vstd!arithmetic.power2.pow2.? (I 18)) 262144)
     (= (vstd!arithmetic.power2.pow2.? (I 19)) 524288)
     (= (vstd!arithmetic.power2.pow2.? (I 20)) 1048576)
     (= (vstd!arithmetic.power2.pow2.? (I 21)) 2097152)
     (= (vstd!arithmetic.power2.pow2.? (I 22)) 4194304)
     (= (vstd!arithmetic.power2.pow2.? (I 23)) 8388608)
     (= (vstd!arithmetic.power2.pow2.? (I 24)) 16777216)
     (= (vstd!arithmetic.power2.pow2.? (I 25)) 33554432)
     (= (vstd!arithmetic.power2.pow2.? (I 26)) 67108864)
     (= (vstd!arithmetic.power2.pow2.? (I 27)) 134217728)
     (= (vstd!arithmetic.power2.pow2.? (I 28)) 268435456)
     (= (vstd!arithmetic.power2.pow2.? (I 29)) 536870912)
     (= (vstd!arithmetic.power2.pow2.? (I 30)) 1073741824)
     (= (vstd!arithmetic.power2.pow2.? (I 31)) 2147483648)
     (= (vstd!arithmetic.power2.pow2.? (I 32)) 4294967296)
     (= (vstd!arithmetic.power2.pow2.? (I 64)) 18446744073709551616)
   ))
   :pattern ((ens%vstd!arithmetic.power2.lemma2_to64. no%param))
   :qid internal_ens__vstd!arithmetic.power2.lemma2_to64._definition
   :skolemid skolem_internal_ens__vstd!arithmetic.power2.lemma2_to64._definition
)))
(assert
 (tr_bound%vstd!endian.BasePow2. $ (UINT 8))
)
(assert
 (tr_bound%vstd!endian.BasePow2. $ (UINT 64))
)
(assert
 (tr_bound%vstd!endian.BasePow2. $ USIZE)
)
(assert
 (tr_bound%vstd!endian.CompatibleSmallerBaseFor. $ (UINT 8) $ (UINT 64))
)
(assert
 (tr_bound%vstd!endian.CompatibleSmallerBaseFor. $ (UINT 8) $ USIZE)
)
(assert
 (forall ((T&. Dcr) (T& Type) (A&. Dcr) (A& Type)) (!
   (=>
    (and
     (sized T&.)
     (sized A&.)
     (tr_bound%core!clone.Clone. T&. T&)
     (tr_bound%core!alloc.Allocator. A&. A&)
     (tr_bound%core!clone.Clone. A&. A&)
    )
    (tr_bound%core!clone.Clone. (BOX A&. A& T&.) T&)
   )
   :pattern ((tr_bound%core!clone.Clone. (BOX A&. A& T&.) T&))
   :qid internal_alloc__boxed__impl&__15_trait_impl_definition
   :skolemid skolem_internal_alloc__boxed__impl&__15_trait_impl_definition
)))
(assert
 (tr_bound%core!clone.Clone. $ BOOL)
)
(assert
 (forall ((T&. Dcr) (T& Type)) (!
   (tr_bound%core!clone.Clone. (REF T&.) T&)
   :pattern ((tr_bound%core!clone.Clone. (REF T&.) T&))
   :qid internal_core__clone__impls__impl&__6_trait_impl_definition
   :skolemid skolem_internal_core__clone__impls__impl&__6_trait_impl_definition
)))
(assert
 (forall ((T&. Dcr) (T& Type)) (!
   (tr_bound%core!clone.Clone. $ (TYPE%core!marker.PhantomData. T&. T&))
   :pattern ((tr_bound%core!clone.Clone. $ (TYPE%core!marker.PhantomData. T&. T&)))
   :qid internal_core__marker__impl&__13_trait_impl_definition
   :skolemid skolem_internal_core__marker__impl&__13_trait_impl_definition
)))
(assert
 (forall ((A&. Dcr) (A& Type)) (!
   (=>
    (sized A&.)
    (tr_bound%core!clone.Clone. (GHOST A&.) A&)
   )
   :pattern ((tr_bound%core!clone.Clone. (GHOST A&.) A&))
   :qid internal_verus_builtin__impl&__7_trait_impl_definition
   :skolemid skolem_internal_verus_builtin__impl&__7_trait_impl_definition
)))
(assert
 (forall ((A&. Dcr) (A& Type)) (!
   (=>
    (and
     (sized A&.)
     (tr_bound%core!marker.Copy. A&. A&)
    )
    (tr_bound%core!clone.Clone. (TRACKED A&.) A&)
   )
   :pattern ((tr_bound%core!clone.Clone. (TRACKED A&.) A&))
   :qid internal_verus_builtin__impl&__9_trait_impl_definition
   :skolemid skolem_internal_verus_builtin__impl&__9_trait_impl_definition
)))
(assert
 (tr_bound%core!marker.Copy. $ (UINT 8))
)
(assert
 (tr_bound%core!marker.Copy. $ (UINT 32))
)
(assert
 (tr_bound%core!marker.Copy. $ (UINT 64))
)
(assert
 (tr_bound%core!marker.Copy. $ BOOL)
)
(assert
 (forall ((T&. Dcr) (T& Type)) (!
   (tr_bound%core!marker.Copy. (REF T&.) T&)
   :pattern ((tr_bound%core!marker.Copy. (REF T&.) T&))
   :qid internal_core__marker__impl&__4_trait_impl_definition
   :skolemid skolem_internal_core__marker__impl&__4_trait_impl_definition
)))
(assert
 (forall ((T&. Dcr) (T& Type)) (!
   (tr_bound%core!marker.Copy. $ (TYPE%core!marker.PhantomData. T&. T&))
   :pattern ((tr_bound%core!marker.Copy. $ (TYPE%core!marker.PhantomData. T&. T&)))
   :qid internal_core__marker__impl&__12_trait_impl_definition
   :skolemid skolem_internal_core__marker__impl&__12_trait_impl_definition
)))
(assert
 (forall ((A&. Dcr) (A& Type)) (!
   (=>
    (sized A&.)
    (tr_bound%core!marker.Copy. (GHOST A&.) A&)
   )
   :pattern ((tr_bound%core!marker.Copy. (GHOST A&.) A&))
   :qid internal_verus_builtin__impl&__8_trait_impl_definition
   :skolemid skolem_internal_verus_builtin__impl&__8_trait_impl_definition
)))
(assert
 (forall ((A&. Dcr) (A& Type)) (!
   (=>
    (and
     (sized A&.)
     (tr_bound%core!marker.Copy. A&. A&)
    )
    (tr_bound%core!marker.Copy. (TRACKED A&.) A&)
   )
   :pattern ((tr_bound%core!marker.Copy. (TRACKED A&.) A&))
   :qid internal_verus_builtin__impl&__10_trait_impl_definition
   :skolemid skolem_internal_verus_builtin__impl&__10_trait_impl_definition
)))
(assert
 (forall ((A&. Dcr) (A& Type) (F&. Dcr) (F& Type)) (!
   (=>
    (and
     (sized A&.)
     (tr_bound%core!marker.Tuple. A&. A&)
     (tr_bound%core!ops.function.Fn. F&. F& A&. A&)
    )
    (tr_bound%core!ops.function.FnOnce. (REF F&.) F& A&. A&)
   )
   :pattern ((tr_bound%core!ops.function.FnOnce. (REF F&.) F& A&. A&))
   :qid internal_core__ops__function__impls__impl&__2_trait_impl_definition
   :skolemid skolem_internal_core__ops__function__impls__impl&__2_trait_impl_definition
)))
(assert
 (forall ((A&. Dcr) (A& Type) (F&. Dcr) (F& Type)) (!
   (=>
    (and
     (sized A&.)
     (tr_bound%core!marker.Tuple. A&. A&)
     (tr_bound%core!ops.function.Fn. F&. F& A&. A&)
    )
    (tr_bound%core!ops.function.FnMut. (REF F&.) F& A&. A&)
   )
   :pattern ((tr_bound%core!ops.function.FnMut. (REF F&.) F& A&. A&))
   :qid internal_core__ops__function__impls__impl&__1_trait_impl_definition
   :skolemid skolem_internal_core__ops__function__impls__impl&__1_trait_impl_definition
)))
(assert
 (forall ((A&. Dcr) (A& Type) (F&. Dcr) (F& Type)) (!
   (=>
    (and
     (sized A&.)
     (tr_bound%core!marker.Tuple. A&. A&)
     (tr_bound%core!ops.function.Fn. F&. F& A&. A&)
    )
    (tr_bound%core!ops.function.Fn. (REF F&.) F& A&. A&)
   )
   :pattern ((tr_bound%core!ops.function.Fn. (REF F&.) F& A&. A&))
   :qid internal_core__ops__function__impls__impl&__0_trait_impl_definition
   :skolemid skolem_internal_core__ops__function__impls__impl&__0_trait_impl_definition
)))
(assert
 (forall ((Args&. Dcr) (Args& Type) (F&. Dcr) (F& Type) (A&. Dcr) (A& Type)) (!
   (=>
    (and
     (sized Args&.)
     (sized A&.)
     (tr_bound%core!marker.Tuple. Args&. Args&)
     (tr_bound%core!ops.function.FnOnce. F&. F& Args&. Args&)
     (tr_bound%core!alloc.Allocator. A&. A&)
    )
    (tr_bound%core!ops.function.FnOnce. (BOX A&. A& F&.) F& Args&. Args&)
   )
   :pattern ((tr_bound%core!ops.function.FnOnce. (BOX A&. A& F&.) F& Args&. Args&))
   :qid internal_alloc__boxed__impl&__31_trait_impl_definition
   :skolemid skolem_internal_alloc__boxed__impl&__31_trait_impl_definition
)))
(assert
 (forall ((Args&. Dcr) (Args& Type) (F&. Dcr) (F& Type) (A&. Dcr) (A& Type)) (!
   (=>
    (and
     (sized Args&.)
     (sized A&.)
     (tr_bound%core!marker.Tuple. Args&. Args&)
     (tr_bound%core!ops.function.FnMut. F&. F& Args&. Args&)
     (tr_bound%core!alloc.Allocator. A&. A&)
    )
    (tr_bound%core!ops.function.FnMut. (BOX A&. A& F&.) F& Args&. Args&)
   )
   :pattern ((tr_bound%core!ops.function.FnMut. (BOX A&. A& F&.) F& Args&. Args&))
   :qid internal_alloc__boxed__impl&__32_trait_impl_definition
   :skolemid skolem_internal_alloc__boxed__impl&__32_trait_impl_definition
)))
(assert
 (forall ((Args&. Dcr) (Args& Type) (F&. Dcr) (F& Type) (A&. Dcr) (A& Type)) (!
   (=>
    (and
     (sized Args&.)
     (sized A&.)
     (tr_bound%core!marker.Tuple. Args&. Args&)
     (tr_bound%core!ops.function.Fn. F&. F& Args&. Args&)
     (tr_bound%core!alloc.Allocator. A&. A&)
    )
    (tr_bound%core!ops.function.Fn. (BOX A&. A& F&.) F& Args&. Args&)
   )
   :pattern ((tr_bound%core!ops.function.Fn. (BOX A&. A& F&.) F& Args&. Args&))
   :qid internal_alloc__boxed__impl&__33_trait_impl_definition
   :skolemid skolem_internal_alloc__boxed__impl&__33_trait_impl_definition
)))
(assert
 (forall ((A&. Dcr) (A& Type) (F&. Dcr) (F& Type)) (!
   (=>
    (and
     (sized A&.)
     (tr_bound%core!marker.Tuple. A&. A&)
     (tr_bound%core!ops.function.FnMut. F&. F& A&. A&)
    )
    (tr_bound%core!ops.function.FnMut. $ (MUTREF F&. F&) A&. A&)
   )
   :pattern ((tr_bound%core!ops.function.FnMut. $ (MUTREF F&. F&) A&. A&))
   :qid internal_core__ops__function__impls__impl&__3_trait_impl_definition
   :skolemid skolem_internal_core__ops__function__impls__impl&__3_trait_impl_definition
)))
(assert
 (forall ((A&. Dcr) (A& Type)) (!
   (=>
    (tr_bound%core!alloc.Allocator. A&. A&)
    (tr_bound%core!alloc.Allocator. (REF A&.) A&)
   )
   :pattern ((tr_bound%core!alloc.Allocator. (REF A&.) A&))
   :qid internal_core__alloc__impl&__2_trait_impl_definition
   :skolemid skolem_internal_core__alloc__impl&__2_trait_impl_definition
)))
(assert
 (forall ((A&. Dcr) (A& Type)) (!
   (=>
    (tr_bound%core!alloc.Allocator. A&. A&)
    (tr_bound%core!alloc.Allocator. $ (MUTREF A&. A&))
   )
   :pattern ((tr_bound%core!alloc.Allocator. $ (MUTREF A&. A&)))
   :qid internal_core__alloc__impl&__3_trait_impl_definition
   :skolemid skolem_internal_core__alloc__impl&__3_trait_impl_definition
)))
(assert
 (forall ((T&. Dcr) (T& Type) (A&. Dcr) (A& Type)) (!
   (=>
    (and
     (sized A&.)
     (tr_bound%core!alloc.Allocator. T&. T&)
     (tr_bound%core!alloc.Allocator. A&. A&)
    )
    (tr_bound%core!alloc.Allocator. (BOX A&. A& T&.) T&)
   )
   :pattern ((tr_bound%core!alloc.Allocator. (BOX A&. A& T&.) T&))
   :qid internal_alloc__boxed__impl&__49_trait_impl_definition
   :skolemid skolem_internal_alloc__boxed__impl&__49_trait_impl_definition
)))
(assert
 (forall ((T&. Dcr) (T& Type) (A&. Dcr) (A& Type)) (!
   (=>
    (and
     (sized A&.)
     (tr_bound%core!alloc.Allocator. T&. T&)
     (tr_bound%core!alloc.Allocator. A&. A&)
    )
    (tr_bound%core!alloc.Allocator. (RC A&. A& T&.) T&)
   )
   :pattern ((tr_bound%core!alloc.Allocator. (RC A&. A& T&.) T&))
   :qid internal_alloc__rc__impl&__116_trait_impl_definition
   :skolemid skolem_internal_alloc__rc__impl&__116_trait_impl_definition
)))
(assert
 (forall ((T&. Dcr) (T& Type) (A&. Dcr) (A& Type)) (!
   (=>
    (and
     (sized A&.)
     (tr_bound%core!alloc.Allocator. T&. T&)
     (tr_bound%core!alloc.Allocator. A&. A&)
    )
    (tr_bound%core!alloc.Allocator. (ARC A&. A& T&.) T&)
   )
   :pattern ((tr_bound%core!alloc.Allocator. (ARC A&. A& T&.) T&))
   :qid internal_alloc__sync__impl&__118_trait_impl_definition
   :skolemid skolem_internal_alloc__sync__impl&__118_trait_impl_definition
)))
(assert
 (tr_bound%verus_builtin!Integer. $ (UINT 8))
)
(assert
 (tr_bound%verus_builtin!Integer. $ (UINT 32))
)
(assert
 (tr_bound%verus_builtin!Integer. $ (UINT 64))
)
(declare-fun req%vstd!arithmetic.div_mod.lemma_div_plus_one. (Int Int) Bool)
(declare-const %%global_location_label%%47 Bool)
(assert
 (forall ((x! Int) (d! Int)) (!
   (= (req%vstd!arithmetic.div_mod.lemma_div_plus_one. x! d!) (=>
     %%global_location_label%%47
     (< 0 d!)
   ))
   :pattern ((req%vstd!arithmetic.div_mod.lemma_div_plus_one. x! d!))
   :qid internal_req__vstd!arithmetic.div_mod.lemma_div_plus_one._definition
   :skolemid skolem_internal_req__vstd!arithmetic.div_mod.lemma_div_plus_one._definition
)))
(declare-fun ens%vstd!arithmetic.div_mod.lemma_div_plus_one. (Int Int) Bool)
(assert
 (forall ((x! Int) (d! Int)) (!
   (= (ens%vstd!arithmetic.div_mod.lemma_div_plus_one. x! d!) (= (Add 1 (EucDiv x! d!))
     (EucDiv (Add d! x!) d!)
   ))
   :pattern ((ens%vstd!arithmetic.div_mod.lemma_div_plus_one. x! d!))
   :qid internal_ens__vstd!arithmetic.div_mod.lemma_div_plus_one._definition
   :skolemid skolem_internal_ens__vstd!arithmetic.div_mod.lemma_div_plus_one._definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!arithmetic.div_mod.lemma_div_plus_one.)
  (forall ((x! Int) (d! Int)) (!
    (=>
     (< 0 d!)
     (= (Add 1 (EucDiv x! d!)) (EucDiv (Add d! x!) d!))
    )
    :pattern ((Add 1 (EucDiv x! d!)) (EucDiv (Add d! x!) d!))
    :qid user_vstd__arithmetic__div_mod__lemma_div_plus_one_0
    :skolemid skolem_user_vstd__arithmetic__div_mod__lemma_div_plus_one_0
))))
(declare-fun req%vstd!arithmetic.div_mod.lemma_mod_self_0. (Int) Bool)
(declare-const %%global_location_label%%48 Bool)
(assert
 (forall ((m! Int)) (!
   (= (req%vstd!arithmetic.div_mod.lemma_mod_self_0. m!) (=>
     %%global_location_label%%48
     (> m! 0)
   ))
   :pattern ((req%vstd!arithmetic.div_mod.lemma_mod_self_0. m!))
   :qid internal_req__vstd!arithmetic.div_mod.lemma_mod_self_0._definition
   :skolemid skolem_internal_req__vstd!arithmetic.div_mod.lemma_mod_self_0._definition
)))
(declare-fun ens%vstd!arithmetic.div_mod.lemma_mod_self_0. (Int) Bool)
(assert
 (forall ((m! Int)) (!
   (= (ens%vstd!arithmetic.div_mod.lemma_mod_self_0. m!) (= (EucMod m! m!) 0))
   :pattern ((ens%vstd!arithmetic.div_mod.lemma_mod_self_0. m!))
   :qid internal_ens__vstd!arithmetic.div_mod.lemma_mod_self_0._definition
   :skolemid skolem_internal_ens__vstd!arithmetic.div_mod.lemma_mod_self_0._definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!arithmetic.div_mod.lemma_mod_self_0.)
  (forall ((m! Int)) (!
    (=>
     (> m! 0)
     (= (EucMod m! m!) 0)
    )
    :pattern ((EucMod m! m!))
    :qid user_vstd__arithmetic__div_mod__lemma_mod_self_0_0
    :skolemid skolem_user_vstd__arithmetic__div_mod__lemma_mod_self_0_0
))))
(declare-fun req%vstd!arithmetic.div_mod.lemma_fundamental_div_mod_converse. (Int Int
  Int Int
 ) Bool
)
(declare-const %%global_location_label%%49 Bool)
(declare-const %%global_location_label%%50 Bool)
(declare-const %%global_location_label%%51 Bool)
(assert
 (forall ((x! Int) (d! Int) (q! Int) (r! Int)) (!
   (= (req%vstd!arithmetic.div_mod.lemma_fundamental_div_mod_converse. x! d! q! r!) (
     and
     (=>
      %%global_location_label%%49
      (not (= d! 0))
     )
     (=>
      %%global_location_label%%50
      (let
       ((tmp%%$ 0))
       (let
        ((tmp%%$1 r!))
        (let
         ((tmp%%$2 d!))
         (and
          (<= tmp%%$ tmp%%$1)
          (< tmp%%$1 tmp%%$2)
     )))))
     (=>
      %%global_location_label%%51
      (= x! (Add (Mul q! d!) r!))
   )))
   :pattern ((req%vstd!arithmetic.div_mod.lemma_fundamental_div_mod_converse. x! d! q!
     r!
   ))
   :qid internal_req__vstd!arithmetic.div_mod.lemma_fundamental_div_mod_converse._definition
   :skolemid skolem_internal_req__vstd!arithmetic.div_mod.lemma_fundamental_div_mod_converse._definition
)))
(declare-fun ens%vstd!arithmetic.div_mod.lemma_fundamental_div_mod_converse. (Int Int
  Int Int
 ) Bool
)
(assert
 (forall ((x! Int) (d! Int) (q! Int) (r! Int)) (!
   (= (ens%vstd!arithmetic.div_mod.lemma_fundamental_div_mod_converse. x! d! q! r!) (
     and
     (= r! (EucMod x! d!))
     (= q! (EucDiv x! d!))
   ))
   :pattern ((ens%vstd!arithmetic.div_mod.lemma_fundamental_div_mod_converse. x! d! q!
     r!
   ))
   :qid internal_ens__vstd!arithmetic.div_mod.lemma_fundamental_div_mod_converse._definition
   :skolemid skolem_internal_ens__vstd!arithmetic.div_mod.lemma_fundamental_div_mod_converse._definition
)))
(declare-fun req%vstd!arithmetic.div_mod.lemma_mod_multiples_vanish. (Int Int Int)
 Bool
)
(declare-const %%global_location_label%%52 Bool)
(assert
 (forall ((a! Int) (b! Int) (m! Int)) (!
   (= (req%vstd!arithmetic.div_mod.lemma_mod_multiples_vanish. a! b! m!) (=>
     %%global_location_label%%52
     (< 0 m!)
   ))
   :pattern ((req%vstd!arithmetic.div_mod.lemma_mod_multiples_vanish. a! b! m!))
   :qid internal_req__vstd!arithmetic.div_mod.lemma_mod_multiples_vanish._definition
   :skolemid skolem_internal_req__vstd!arithmetic.div_mod.lemma_mod_multiples_vanish._definition
)))
(declare-fun ens%vstd!arithmetic.div_mod.lemma_mod_multiples_vanish. (Int Int Int)
 Bool
)
(assert
 (forall ((a! Int) (b! Int) (m! Int)) (!
   (= (ens%vstd!arithmetic.div_mod.lemma_mod_multiples_vanish. a! b! m!) (= (EucMod (Add
       (Mul m! a!) b!
      ) m!
     ) (EucMod b! m!)
   ))
   :pattern ((ens%vstd!arithmetic.div_mod.lemma_mod_multiples_vanish. a! b! m!))
   :qid internal_ens__vstd!arithmetic.div_mod.lemma_mod_multiples_vanish._definition
   :skolemid skolem_internal_ens__vstd!arithmetic.div_mod.lemma_mod_multiples_vanish._definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!arithmetic.div_mod.lemma_mod_multiples_vanish.)
  (forall ((a! Int) (b! Int) (m! Int)) (!
    (=>
     (< 0 m!)
     (= (EucMod (Add (Mul m! a!) b!) m!) (EucMod b! m!))
    )
    :pattern ((EucMod (Add (Mul m! a!) b!) m!))
    :qid user_vstd__arithmetic__div_mod__lemma_mod_multiples_vanish_0
    :skolemid skolem_user_vstd__arithmetic__div_mod__lemma_mod_multiples_vanish_0
))))
(declare-fun req%vstd!arithmetic.div_mod.lemma_mod_multiples_basic. (Int Int) Bool)
(declare-const %%global_location_label%%53 Bool)
(assert
 (forall ((x! Int) (m! Int)) (!
   (= (req%vstd!arithmetic.div_mod.lemma_mod_multiples_basic. x! m!) (=>
     %%global_location_label%%53
     (> m! 0)
   ))
   :pattern ((req%vstd!arithmetic.div_mod.lemma_mod_multiples_basic. x! m!))
   :qid internal_req__vstd!arithmetic.div_mod.lemma_mod_multiples_basic._definition
   :skolemid skolem_internal_req__vstd!arithmetic.div_mod.lemma_mod_multiples_basic._definition
)))
(declare-fun ens%vstd!arithmetic.div_mod.lemma_mod_multiples_basic. (Int Int) Bool)
(assert
 (forall ((x! Int) (m! Int)) (!
   (= (ens%vstd!arithmetic.div_mod.lemma_mod_multiples_basic. x! m!) (= (EucMod (Mul x!
       m!
      ) m!
     ) 0
   ))
   :pattern ((ens%vstd!arithmetic.div_mod.lemma_mod_multiples_basic. x! m!))
   :qid internal_ens__vstd!arithmetic.div_mod.lemma_mod_multiples_basic._definition
   :skolemid skolem_internal_ens__vstd!arithmetic.div_mod.lemma_mod_multiples_basic._definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!arithmetic.div_mod.lemma_mod_multiples_basic.)
  (forall ((x! Int) (m! Int)) (!
    (=>
     (> m! 0)
     (= (EucMod (Mul x! m!) m!) 0)
    )
    :pattern ((EucMod (Mul x! m!) m!))
    :qid user_vstd__arithmetic__div_mod__lemma_mod_multiples_basic_0
    :skolemid skolem_user_vstd__arithmetic__div_mod__lemma_mod_multiples_basic_0
))))
(declare-fun req%vstd!arithmetic.div_mod.lemma_mod_sub_multiples_vanish. (Int Int)
 Bool
)
(declare-const %%global_location_label%%54 Bool)
(assert
 (forall ((b! Int) (m! Int)) (!
   (= (req%vstd!arithmetic.div_mod.lemma_mod_sub_multiples_vanish. b! m!) (=>
     %%global_location_label%%54
     (< 0 m!)
   ))
   :pattern ((req%vstd!arithmetic.div_mod.lemma_mod_sub_multiples_vanish. b! m!))
   :qid internal_req__vstd!arithmetic.div_mod.lemma_mod_sub_multiples_vanish._definition
   :skolemid skolem_internal_req__vstd!arithmetic.div_mod.lemma_mod_sub_multiples_vanish._definition
)))
(declare-fun ens%vstd!arithmetic.div_mod.lemma_mod_sub_multiples_vanish. (Int Int)
 Bool
)
(assert
 (forall ((b! Int) (m! Int)) (!
   (= (ens%vstd!arithmetic.div_mod.lemma_mod_sub_multiples_vanish. b! m!) (= (EucMod (Add
       (Sub 0 m!) b!
      ) m!
     ) (EucMod b! m!)
   ))
   :pattern ((ens%vstd!arithmetic.div_mod.lemma_mod_sub_multiples_vanish. b! m!))
   :qid internal_ens__vstd!arithmetic.div_mod.lemma_mod_sub_multiples_vanish._definition
   :skolemid skolem_internal_ens__vstd!arithmetic.div_mod.lemma_mod_sub_multiples_vanish._definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!arithmetic.div_mod.lemma_mod_sub_multiples_vanish.)
  (forall ((b! Int) (m! Int)) (!
    (=>
     (< 0 m!)
     (= (EucMod (Add (Sub 0 m!) b!) m!) (EucMod b! m!))
    )
    :pattern ((EucMod b! m!))
    :qid user_vstd__arithmetic__div_mod__lemma_mod_sub_multiples_vanish_0
    :skolemid skolem_user_vstd__arithmetic__div_mod__lemma_mod_sub_multiples_vanish_0
))))
(declare-fun ens%vstd!arithmetic.power.lemma_pow1. (Int) Bool)
(assert
 (forall ((b! Int)) (!
   (= (ens%vstd!arithmetic.power.lemma_pow1. b!) (= (vstd!arithmetic.power.pow.? (I b!)
      (I 1)
     ) b!
   ))
   :pattern ((ens%vstd!arithmetic.power.lemma_pow1. b!))
   :qid internal_ens__vstd!arithmetic.power.lemma_pow1._definition
   :skolemid skolem_internal_ens__vstd!arithmetic.power.lemma_pow1._definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!arithmetic.power.lemma_pow1.)
  (forall ((b! Poly)) (!
    (=>
     (has_type b! INT)
     (= (vstd!arithmetic.power.pow.? b! (I 1)) (%I b!))
    )
    :pattern ((vstd!arithmetic.power.pow.? b! (I 1)))
    :qid user_vstd__arithmetic__power__lemma_pow1_0
    :skolemid skolem_user_vstd__arithmetic__power__lemma_pow1_0
))))
(declare-fun req%vstd!arithmetic.power.lemma_pow_sub_add_cancel. (Int Int Int) Bool)
(declare-const %%global_location_label%%55 Bool)
(assert
 (forall ((b! Int) (e1! Int) (e2! Int)) (!
   (= (req%vstd!arithmetic.power.lemma_pow_sub_add_cancel. b! e1! e2!) (=>
     %%global_location_label%%55
     (>= e1! e2!)
   ))
   :pattern ((req%vstd!arithmetic.power.lemma_pow_sub_add_cancel. b! e1! e2!))
   :qid internal_req__vstd!arithmetic.power.lemma_pow_sub_add_cancel._definition
   :skolemid skolem_internal_req__vstd!arithmetic.power.lemma_pow_sub_add_cancel._definition
)))
(declare-fun ens%vstd!arithmetic.power.lemma_pow_sub_add_cancel. (Int Int Int) Bool)
(assert
 (forall ((b! Int) (e1! Int) (e2! Int)) (!
   (= (ens%vstd!arithmetic.power.lemma_pow_sub_add_cancel. b! e1! e2!) (= (Mul (vstd!arithmetic.power.pow.?
       (I b!) (I (nClip (Sub e1! e2!)))
      ) (vstd!arithmetic.power.pow.? (I b!) (I e2!))
     ) (vstd!arithmetic.power.pow.? (I b!) (I e1!))
   ))
   :pattern ((ens%vstd!arithmetic.power.lemma_pow_sub_add_cancel. b! e1! e2!))
   :qid internal_ens__vstd!arithmetic.power.lemma_pow_sub_add_cancel._definition
   :skolemid skolem_internal_ens__vstd!arithmetic.power.lemma_pow_sub_add_cancel._definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!arithmetic.power.lemma_pow_sub_add_cancel.)
  (forall ((b! Poly) (e1! Int) (e2! Int)) (!
    (=>
     (and
      (has_type b! INT)
      (<= 0 e1!)
      (<= 0 e2!)
     )
     (=>
      (>= e1! e2!)
      (= (Mul (vstd!arithmetic.power.pow.? b! (I (nClip (Sub e1! e2!)))) (vstd!arithmetic.power.pow.?
         b! (I e2!)
        )
       ) (vstd!arithmetic.power.pow.? b! (I e1!))
    )))
    :pattern ((vstd!arithmetic.power.pow.? b! (I (nClip (Sub e1! e2!)))))
    :qid user_vstd__arithmetic__power__lemma_pow_sub_add_cancel_0
    :skolemid skolem_user_vstd__arithmetic__power__lemma_pow_sub_add_cancel_0
))))
(declare-fun req%vstd!endian.impl&%0.to_nat_properties. (Dcr Type vstd!endian.EndianNat.)
 Bool
)
(declare-const %%global_location_label%%56 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (self! vstd!endian.EndianNat.)) (!
   (= (req%vstd!endian.impl&%0.to_nat_properties. B&. B& self!) (=>
     %%global_location_label%%56
     (vstd!endian.impl&%0.wf.? B&. B& (Poly%vstd!endian.EndianNat. self!))
   ))
   :pattern ((req%vstd!endian.impl&%0.to_nat_properties. B&. B& self!))
   :qid internal_req__vstd!endian.impl&__0.to_nat_properties._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.to_nat_properties._definition
)))
(declare-fun ens%vstd!endian.impl&%0.to_nat_properties. (Dcr Type vstd!endian.EndianNat.)
 Bool
)
(assert
 (forall ((B&. Dcr) (B& Type) (self! vstd!endian.EndianNat.)) (!
   (= (ens%vstd!endian.impl&%0.to_nat_properties. B&. B& self!) (< (vstd!endian.impl&%0.to_nat.?
      B&. B& (Poly%vstd!endian.EndianNat. self!)
     ) (vstd!endian.base_upper_bound_excl.? B&. B& (I (vstd!endian.impl&%0.len.? B&. B& (
         Poly%vstd!endian.EndianNat. self!
   ))))))
   :pattern ((ens%vstd!endian.impl&%0.to_nat_properties. B&. B& self!))
   :qid internal_ens__vstd!endian.impl&__0.to_nat_properties._definition
   :skolemid skolem_internal_ens__vstd!endian.impl&__0.to_nat_properties._definition
)))
(assert
 (=>
  (fuel_bool fuel%vstd!endian.impl&%0.to_nat_properties.)
  (forall ((B&. Dcr) (B& Type) (self! Poly)) (!
    (=>
     (has_type self! (TYPE%vstd!endian.EndianNat. B&. B&))
     (=>
      (and
       (and
        (sized B&.)
        (tr_bound%vstd!endian.Base. B&. B&)
       )
       (vstd!endian.impl&%0.wf.? B&. B& self!)
      )
      (< (vstd!endian.impl&%0.to_nat.? B&. B& self!) (vstd!endian.base_upper_bound_excl.?
        B&. B& (I (vstd!endian.impl&%0.len.? B&. B& self!))
    ))))
    :pattern ((vstd!endian.impl&%0.to_nat.? B&. B& self!))
    :qid user_vstd__endian__EndianNat__to_nat_properties_0
    :skolemid skolem_user_vstd__endian__EndianNat__to_nat_properties_0
))))
(declare-fun req%vstd!endian.impl&%0.to_nat_append_least. (Dcr Type vstd!endian.EndianNat.
  vstd!endian.EndianNat.
 ) Bool
)
(declare-const %%global_location_label%%57 Bool)
(declare-const %%global_location_label%%58 Bool)
(declare-const %%global_location_label%%59 Bool)
(assert
 (forall ((B&. Dcr) (B& Type) (endian! vstd!endian.EndianNat.) (other! vstd!endian.EndianNat.))
  (!
   (= (req%vstd!endian.impl&%0.to_nat_append_least. B&. B& endian! other!) (and
     (=>
      %%global_location_label%%57
      (vstd!endian.impl&%0.wf.? B&. B& (Poly%vstd!endian.EndianNat. endian!))
     )
     (=>
      %%global_location_label%%58
      (vstd!endian.impl&%0.wf.? B&. B& (Poly%vstd!endian.EndianNat. other!))
     )
     (=>
      %%global_location_label%%59
      (= (vstd!endian.EndianNat./EndianNat/endian (%Poly%vstd!endian.EndianNat. (Poly%vstd!endian.EndianNat.
          endian!
        ))
       ) (vstd!endian.EndianNat./EndianNat/endian (%Poly%vstd!endian.EndianNat. (Poly%vstd!endian.EndianNat.
          other!
   )))))))
   :pattern ((req%vstd!endian.impl&%0.to_nat_append_least. B&. B& endian! other!))
   :qid internal_req__vstd!endian.impl&__0.to_nat_append_least._definition
   :skolemid skolem_internal_req__vstd!endian.impl&__0.to_nat_append_least._definition
)))
(declare-fun ens%vstd!endian.impl&%0.to_nat_append_least. (Dcr Type vstd!endian.EndianNat.
  vstd!endian.EndianNat.
 ) Bool
)
(assert
 (forall ((B&. Dcr) (B& Type) (endian! vstd!endian.EndianNat.) (other! vstd!endian.EndianNat.))
  (!
   (= (ens%vstd!endian.impl&%0.to_nat_append_least. B&. B& endian! other!) (= (vstd!endian.impl&%0.to_nat.?
      B&. B& (Poly%vstd!endian.EndianNat. (vstd!endian.impl&%0.append_least.? B&. B& (Poly%vstd!endian.EndianNat.
         endian!
        ) (Poly%vstd!endian.EndianNat. other!)
      ))
     ) (Add (Mul (vstd!endian.impl&%0.to_nat.? B&. B& (Poly%vstd!endian.EndianNat. endian!))
       (vstd!endian.base_upper_bound_excl.? B&. B& (I (vstd!endian.impl&%0.len.? B&. B& (Poly%vstd!endian.EndianNat.
           other!
       ))))
      ) (vstd!endian.impl&%0.to_nat.? B&. B& (Poly%vstd!endian.EndianNat. other!))
   )))
   :pattern ((ens%vstd!endian.impl&%0.to_nat_append_least. B&. B& endian! other!))
   :qid internal_ens__vstd!endian.impl&__0.to_nat_append_least._definition
   :skolemid skolem_internal_ens__vstd!endian.impl&__0.to_nat_append_least._definition
)))
(set-option :smt.arith.solver 6)
(push)
(get-info :all-statistics)

(echo "<<DONE>>")
(declare-const B&. Dcr)
(declare-const B& Type)
(declare-const endian! vstd!endian.EndianNat.)
(declare-const other! vstd!endian.EndianNat.)
(declare-const tmp%1 Bool)
(declare-const tmp%2 Bool)
(declare-const tmp%3 Bool)
(declare-const least@ Int)
(declare-const rest@ vstd!endian.EndianNat.)
(declare-const decrease%init0 Int)
(assert
 fuel_defaults
)
(assert
 (sized B&.)
)
(assert
 (tr_bound%vstd!endian.Base. B&. B&)
)
(declare-const %%location_label%%0 Bool)
(assert
 (not (=>
   (= (Mul (vstd!endian.base_upper_bound_excl.? B&. B& (I (vstd!endian.impl&%0.len.? B&.
        B& (Poly%vstd!endian.EndianNat. rest@)
      ))
     ) (%I (vstd!endian.Base.base.? B&. B&))
    ) (vstd!endian.base_upper_bound_excl.? B&. B& (I (nClip (Add (vstd!endian.impl&%0.len.?
         B&. B& (Poly%vstd!endian.EndianNat. rest@)
        ) 1
   )))))
   (=>
    %%location_label%%0
    (= (Mul (Mul (vstd!endian.impl&%0.to_nat.? B&. B& (Poly%vstd!endian.EndianNat. endian!))
       (vstd!endian.base_upper_bound_excl.? B&. B& (I (vstd!endian.impl&%0.len.? B&. B& (Poly%vstd!endian.EndianNat.
           rest@
       ))))
      ) (%I (vstd!endian.Base.base.? B&. B&))
     ) (Mul (vstd!endian.impl&%0.to_nat.? B&. B& (Poly%vstd!endian.EndianNat. endian!))
      (vstd!endian.base_upper_bound_excl.? B&. B& (I (nClip (Add (vstd!endian.impl&%0.len.?
           B&. B& (Poly%vstd!endian.EndianNat. rest@)
          ) 1
))))))))))
(get-info :all-statistics)

(echo "<<DONE>>")
(get-info :version)

(echo "<<DONE>>")
(set-option :rlimit 30000000)
(check-sat)

(echo "<<DONE>>")
(set-option :rlimit 0)
(get-info :all-statistics)

(echo "<<DONE>>")
