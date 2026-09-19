export let csrf = '';
export function setCSRF(value: string) {
  csrf = value;
}
export class ApiError extends Error {
  constructor(
    public code: string,
    message: string,
    public details: unknown
  ) {
    super(message);
  }
}
export async function request(
  path: string,
  method = 'GET',
  data?: unknown,
  idempotency?: string
): Promise<any> {
  const response = await fetch(path, {
    method,
    credentials: 'same-origin',
    headers: {
      'Content-Type': 'application/json',
      'X-CSRF-Token': csrf,
      ...(idempotency ? { 'Idempotency-Key': idempotency } : {}),
    },
    ...(data === undefined ? {} : { body: JSON.stringify(data) }),
  });
  const value = await response.json();
  if (!response.ok)
    throw new ApiError(
      value.error?.code || 'ERROR',
      value.error?.message || '请求失败',
      value.error?.details
    );
  return value;
}
function decode(value: string): ArrayBuffer {
  const str = atob(value.replace(/-/g, '+').replace(/_/g, '/'));
  return Uint8Array.from(str, (c) => c.charCodeAt(0)).buffer;
}
function encode(value: ArrayBuffer): string {
  return btoa(String.fromCharCode(...new Uint8Array(value)))
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=+$/, '');
}
export async function passkey(
  kind: 'login' | 'register' | 'reauth',
  remember = false
) {
  if (!window.PublicKeyCredential || !navigator.credentials)
    throw new Error(
      '此浏览器不支持 Passkey，请使用支持 WebAuthn 的安全浏览器。'
    );
  const options = await request(`/auth/${kind}/begin`, 'POST', {
    remember_30d: remember,
  });
  const pk = options.publicKey;
  pk.challenge = decode(pk.challenge);
  for (const key of ['allowCredentials', 'excludeCredentials'])
    if (pk[key])
      pk[key] = pk[key].map((v: any) => ({ ...v, id: decode(v.id) }));
  let credential: PublicKeyCredential | null;
  if (kind === 'register') {
    pk.user.id = decode(pk.user.id);
    credential = (await navigator.credentials.create({
      publicKey: pk,
    })) as PublicKeyCredential;
  } else
    credential = (await navigator.credentials.get({
      publicKey: pk,
    })) as PublicKeyCredential;
  if (!credential) throw new Error('已取消，请重新点击登录');
  const r = credential.response;
  const response: Record<string, unknown> = {
    clientDataJSON: encode(r.clientDataJSON),
  };
  if (r instanceof AuthenticatorAttestationResponse) {
    response.attestationObject = encode(r.attestationObject);
    response.transports = r.getTransports?.() || [];
  } else {
    const a = r as AuthenticatorAssertionResponse;
    response.authenticatorData = encode(a.authenticatorData);
    response.signature = encode(a.signature);
    response.userHandle = a.userHandle ? encode(a.userHandle) : null;
  }
  await request(`/auth/${kind}/finish`, 'POST', {
    ceremony_id: options.ceremony_id,
    credential: {
      id: credential.id,
      rawId: encode(credential.rawId),
      type: credential.type,
      response,
      extensions: credential.getClientExtensionResults(),
    },
  });
}
