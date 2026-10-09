## 2. Divisibility: translate the symbol into an equation

The notation $a\mid b$, read “$a$ divides $b$,” means that **there is an integer $k$ such that $b=ak$**.

For example, $4\mid20$ because $20=4\cdot5$. But $4\nmid18$, because no integer multiplied by 4 gives 18.

Some useful consequences follow directly:

- $a\mid0$ since $0=a\cdot0$.
- $1\mid a$ since $a=1\cdot a$.
- If $a\mid b$ and $b\mid c$, then $a\mid c$: write $b=ak$ and $c=b\ell=a(k\ell)$.
- If $d\mid a$ and $d\mid b$, then $d\mid(sa+tb)$ for any integers $s,t$.

The last fact is important. If $a=dx$ and $b=dy$, then

\[sa+tb=s(dx)+t(dy)=d(sx+ty).\]

So every common divisor of $a$ and $b$ divides every integer combination $sa+tb$.

\[\boxed{a\text{ has an inverse modulo }n\iff\gcd(a,n)=1.}\]

## What you should be able to do now

For these lectures, the practical target is to be able to:

1. Turn $a\mid b$ into the equation $b=ak$ and prove things with it.
2. Write a valid induction proof with a base case and induction step.
3. Prove a statement using the least-number principle, including the equation $a=bq+r$ with $0\le r<b$.
4. Run the Euclidean algorithm and identify the last nonzero remainder.
5. Work backwards to express a greatest common divisor as $sa+tb$.
6. Compute remainders, including negative integers such as $-23$ divided by $7$.
7. Find a modular inverse using Bézout coefficients, or prove that none exists.

A reality check: if you can do those seven things and explain why each method works, you have the substance of the first three lectures. You do not need to memorize the long prose proofs word for word.

Try these without looking back:

- Find the remainder when $-23$ is divided by $7$.
- Compute $\gcd(84,30)$ and express it as a linear combination.
- Find the inverse of $14$ modulo $25$.

Answers: the remainder is $5$; $\gcd(84,30)=6=3(30)-1(84)$; and $14^{-1}\equiv9\pmod{25}$, because $14\cdot9=126\equiv1\pmod{25}$.

Read [the lecture notes](https://example.com/notes) and [**more details**](https://example.com/details) beside $x^2$. Keep `$literal$`, `\(code\)`, $5 and $10 readable.

\[\text{divisibility}\;\longrightarrow\;\gcd\text{ and Bézout}\;\longrightarrow\;\text{Euclidean algorithm}\;\longrightarrow\;\text{arithmetic modulo }n\;\longrightarrow\;\text{modular inverses.}\]

| Expression | Meaning |
| --- | --- |
| $\frac{a}{b}$ | A fraction |
| $\begin{pmatrix}a&b\\c&d\end{pmatrix}$ | A matrix |

Inline roots $\sqrt{x}$ and sums $\sum_{i=1}^n i$ should align with the prose. A tall fraction $\frac{1}{1+\frac{1}{x}}$ should increase the line height without overlapping the next line.

Streaming source remains readable: $\frac{a}{$.
