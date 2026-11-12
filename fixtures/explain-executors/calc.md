# X-E: distinct named people

ADR 0011: p runs Clerk.enter and q runs Worker.run. They release on finish.
initial0/queue1/start2(p)/advance3(t1)/finish4(entry=ok,p)/join5/queue6/
start7(q)/advance8(t2)/finish9(x=ok,q)/join10/end11.
All three clauses satisfied, no counterexamples, diagnostics [].
entry line9 col5 byte388, x line10 col5 byte414. Clause lines17/18/19.
No latent/observed values; resource holders carry named executor rather
than treating Actor names as distinct people. No replay candidate.
