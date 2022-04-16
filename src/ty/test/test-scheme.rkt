#lang racket
(require redex/reduction-semantics
         "hook.rkt"
         "../grammar.rkt"
         "../scheme.rkt"
         "../inequalities.rkt"
         "../../logic/substitution.rkt"
         "../../logic/instantiate.rkt"
         )

(module+ test
  (redex-let*
   formality-ty
   [((Env_1 () (Ty_T)) (term (instantiate-quantified EmptyEnv (ForAll ((TyKind T)) ()))))
    ((Env_2 () (Ty_A Ty_B)) (term (instantiate-quantified Env_1 (Exists ((TyKind A) (TyKind B)) ()))))
    ((Env_3 () (Lt_I Lt_J Lt_K Lt_L)) (term (instantiate-quantified Env_2 (Exists ((LtKind I) (LtKind J) (LtKind K) (LtKind L)) ()))))
    (Env_4 (term (env-with-var-mapped-to Env_3 Ty_A (TyRigid (Ref ()) (Lt_I (scalar-ty i32))))))
    (Env_5 (term (env-with-var-mapped-to Env_4 Ty_B (TyRigid (Ref ()) (Lt_J (scalar-ty i32))))))
    (Env_6 (term (env-with-var-related-to-parameter Env_5 Lt_I <= Lt_K)))
    (Env_7 (term (env-with-var-related-to-parameter Env_6 Lt_J <= Lt_K)))
    (Env_8 (term (env-with-var-related-to-parameter Env_7 Lt_K <= Lt_L)))
    (Env (term Env_8))
    ]

   (test-equal
    (term (extract-scheme Env (Ty_A Ty_B)))
    (term (Exists
           ((LtKind Lt_J)
            (LtKind Lt_I)
            (LtKind Lt_K)
            (LtKind Lt_L))
           (Implies
            ((Lt_J <= Lt_K) (Lt_I <= Lt_K) (Lt_K <= Lt_L))
            ((TyRigid (Ref ()) (Lt_I (TyRigid i32 ())))
             (TyRigid (Ref ()) (Lt_J (TyRigid i32 ())))))))
    )
   )
  )