/** How the person proves it is them again: a code sent to their email, or their password. */
export type Factor = { strategy: 'email_code'; to: string } | { strategy: 'password' };

/** The proof being asked for, while the server wants a sign-in from the last ten minutes. */
export type Verify = { factor: Factor; entry: string; checking: boolean; problem?: string };

export type VerifyAction =
  | { type: 'ask'; factor: Factor }
  | { type: 'entered'; entry: string }
  | { type: 'checking' }
  | { type: 'rejected'; problem: string }
  | { type: 'done' };

/** Steps through proving it is them: asked, typing, checking, a wrong entry cleared with why, then done. */
export function verifying(state: Verify | undefined, action: VerifyAction): Verify | undefined {
  switch (action.type) {
    case 'ask':
      return { factor: action.factor, entry: '', checking: false };
    case 'entered':
      return state && !state.checking ? { ...state, entry: action.entry, problem: undefined } : state;
    case 'checking':
      return state ? { ...state, checking: true, problem: undefined } : state;
    case 'rejected':
      return state ? { ...state, entry: '', checking: false, problem: action.problem } : state;
    case 'done':
      return undefined;
  }
}

/**
 * What Clerk's `useReverification` looks for in a request's result before it asks the person to verify
 * and then retries: first factor, within the server's ten minutes.
 */
export const REVERIFY_HINT = {
  clerk_error: {
    type: 'forbidden',
    reason: 'reverification-error',
    metadata: { reverification: { level: 'first_factor', afterMinutes: 10 } },
  },
} as const;

/** A password is quickest for an account that has one; anyone else gets a code at their email. */
export function chooseFactor(
  supported: readonly ({ strategy: string } & { emailAddressId?: string; safeIdentifier?: string })[],
): { factor: Factor; emailAddressId?: string } | undefined {
  if (supported.some((factor) => factor.strategy === 'password')) return { factor: { strategy: 'password' } };
  const email = supported.find((factor) => factor.strategy === 'email_code' && factor.emailAddressId);
  if (email?.emailAddressId)
    return { factor: { strategy: 'email_code', to: email.safeIdentifier ?? 'your email' }, emailAddressId: email.emailAddressId };
  return undefined;
}
