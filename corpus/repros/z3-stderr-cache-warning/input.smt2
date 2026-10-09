(declare-fun f (Int) Int)
(assert (forall ((x Int) (y Int)) (! (= (f x) y) :pattern ((f x)))))
(check-sat)
(exit)
