# vertex-ai-oauth

[

![License: Custom](https://img.shields.io/badge/License-Custom-blue.svg)

](./LICENSE)

OAuth 2.0 utility for Vertex AI Gemini — browser & Node.js, **no service account required**.

---

## Why

Every existing Vertex AI integration requires a service account JSON key. That means key rotation risk, leakage risk, and a setup process non-technical users simply cannot navigate.

This library uses **Google Identity Services (GIS)** to authenticate the end-user directly in the browser via OAuth 2.0. No service account. No backend credential storage. Just a GCP Project ID and a Google login.

> As far as publicly known, no prior open-source implementation exists that uses a user-side GIS OAuth flow to call the raw Vertex AI `streamGenerateContent` endpoint directly from a browser-side chat interface without any service account.

---

## Features

- **Browser**: GIS (`google.accounts.oauth2`) interactive & silent token flow
- **Token auto-refresh**: silent refresh 5 minutes before expiry
- **localStorage persistence**: survives page reloads
- **SSE streaming**: direct `streamGenerateContent` calls with `ReadableStream`
- **Node.js support**: inject any token source via `getToken` callback (ADC, service account, etc.)
- **Gemini 3 thinking models**: automatic `thinkingConfig` injection (excludes gemini-3.1)
- **Universal module**: works as browser `<script>` tag or CommonJS `require()`

---

## Files

| File | Description |
|---|---|
| `lib/vertex-ai-oauth.js` | Universal — browser + Node.js. Supports `getToken`, `refreshToken`, custom storage. |
| `lib/vertex-ai-oauth.browser.js` | Browser-only lightweight version. GIS + localStorage only. |

Use `vertex-ai-oauth.browser.js` for plain HTML pages.
Use `vertex-ai-oauth.js` for bundled apps or Node.js.

---

## Browser Usage

```html
<script src="https://accounts.google.com/gsi/client" async></script>
<script src="lib/vertex-ai-oauth.browser.js"></script>
<script>
  const auth = new VertexAIOAuth({
    clientId:  'YOUR_CLIENT_ID.apps.googleusercontent.com',
    projectId: 'your-gcp-project',
    region:    'us-central1',
    model:     'gemini-2.5-flash',
  });

  auth.onStatusChange = (s) =>
    console.log(s.connected ? `Connected (${s.minutesLeft}m left)` : 'Not connected');

  // Try to restore token from localStorage on page load
  auth.tryAutoSignIn();

  document.getElementById('login-btn').onclick = () => auth.signIn();

  async function sendMessage(userText) {
    const contents = [{ role: 'user', parts: [{ text: userText }] }];
    const response = await auth.stream(contents, 'You are a helpful assistant.', [], null);
    // response is a streaming fetch Response — read with ReadableStream
  }
</script>
```

---

## Node.js Usage

```javascript
const { VertexAIOAuth } = require('./lib/vertex-ai-oauth.js');
const { GoogleAuth } = require('google-auth-library');

const gauth = new GoogleAuth({
  scopes: ['https://www.googleapis.com/auth/cloud-platform'],
});

const auth = new VertexAIOAuth({
  projectId: 'my-project',
  region:    'us-central1',
  model:     'gemini-2.5-flash',
  getToken: async () => {
    const client = await gauth.getClient();
    const { token } = await client.getAccessToken();
    return { accessToken: token, expiresIn: 3600 };
  },
});

const text = await auth.collect(
  [{ role: 'user', parts: [{ text: 'Hello!' }] }],
  'You are a helpful assistant.',
  [],
  null
);
console.log(text);
```

---

## GCP Setup

1. Go to [Google Cloud Console](https://console.cloud.google.com/)
2. Enable **Vertex AI API** on your project
3. Go to **APIs & Services → OAuth consent screen**
   - User type: External
   - Add your email as a test user
4. Go to **APIs & Services → Credentials → Create OAuth 2.0 Client ID**
   - Application type: Web application
   - Authorized JavaScript origins: your domain (e.g. `https://yourdomain.github.io`)
5. Copy the **Client ID** — this goes into `VertexAIOAuth({ clientId: '...' })`
6. Make sure your Google account has the `roles/aiplatform.user` IAM role on the project

---

## Security Notes

- The OAuth access token is stored in `localStorage` as plaintext
- Do not use alongside untrusted third-party scripts (XSS risk)
- Token scope is `https://www.googleapis.com/auth/cloud-platform` — handle with care
- Token expires after 1 hour by default; auto-refresh handles renewal silently
- `clientSecret` is for server-side use only — **never include in browser code**

---

## License

[MIT](./LICENSE) © 2026 shittim-plana
