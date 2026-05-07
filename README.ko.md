# vertex-ai-oauth

[![License: Custom](https://img.shields.io/badge/License-Custom-blue.svg)
](./LICENSE)

Vertex AI Gemini용 OAuth 2.0 유틸리티 — 브라우저 & Node.js, **서비스 계정 불필요**.

---

## 왜 만들었나

기존 Vertex AI 연동 방식은 거의 전부 서비스 계정 JSON 키를 요구합니다. 키 유출 위험, 교체 부담, 그리고 비개발자가 따라가기 어려운 설정 과정이 문제입니다.

이 라이브러리는 **Google Identity Services(GIS)** 를 사용해 브라우저에서 직접 OAuth 2.0으로 인증합니다. 서비스 계정 없음. 백엔드 자격증명 저장 없음. GCP 프로젝트 ID와 Google 로그인만 있으면 됩니다.

> 공개된 자료 기준으로, 브라우저 사이드 GIS OAuth flow를 사용해 서비스 계정 없이 Vertex AI `streamGenerateContent` 엔드포인트를 직접 호출하는 채팅 인터페이스의 선행 오픈소스 구현은 확인되지 않았습니다.

---

## 주요 기능

- **브라우저**: GIS(`google.accounts.oauth2`) 대화형 & silent 토큰 발급
- **토큰 자동 갱신**: 만료 5분 전 silent refresh
- **localStorage 영속화**: 페이지 새로고침 후에도 토큰 유지
- **SSE 스트리밍**: `streamGenerateContent` 직접 호출, `ReadableStream` 지원
- **Node.js 지원**: `getToken` 콜백으로 ADC, 서비스 계정 등 연동 가능
- **Gemini 3 thinking 모델**: `thinkingConfig` 자동 주입 (gemini-3.1 제외)
- **유니버설 모듈**: 브라우저 `<script>` 태그 및 CommonJS `require()` 모두 지원

---

## 파일 구성

| 파일 | 설명 |
|---|---|
| `lib/vertex-ai-oauth.js` | 유니버설 — 브라우저 + Node.js. `getToken`, `refreshToken`, 커스텀 스토리지 지원. |
| `lib/vertex-ai-oauth.browser.js` | 브라우저 전용 경량 버전. GIS + localStorage만 사용. |
| `lib/vertex-ai-oauth-server.js` | 서버 전용. Authorization Code Flow, 토큰 갱신/폐기, GCP 프로젝트 목록 조회. |
| `lib/vertex-ai-oauth-postgresql.js` | 서버 전용. PostgreSQL 토큰 저장 + 자동 갱신. Next.js/Express 백엔드용. |

순수 HTML 페이지에는 `vertex-ai-oauth.browser.js`를 사용하세요.
번들 앱이나 Node.js 환경에는 `vertex-ai-oauth.js`를 사용하세요.

---

## 브라우저 사용 예시

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
    console.log(s.connected ? `연결됨 (${s.minutesLeft}분 남음)` : '미연결');

  // 페이지 로드 시 localStorage에서 토큰 복원 시도
  auth.tryAutoSignIn();

  document.getElementById('login-btn').onclick = () => auth.signIn();

  async function sendMessage(userText) {
    const contents = [{ role: 'user', parts: [{ text: userText }] }];
    const response = await auth.stream(contents, '당신은 친절한 AI 어시스턴트입니다.', [], null);
    // response는 스트리밍 fetch Response — ReadableStream으로 직접 읽습니다
  }
</script>
```

---

## Node.js 사용 예시

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
  [{ role: 'user', parts: [{ text: '안녕하세요!' }] }],
  '당신은 친절한 AI 어시스턴트입니다.',
  [],
  null
);
console.log(text);
```

---

## GCP 설정 방법

1. [Google Cloud Console](https://console.cloud.google.com/) 접속
2. 프로젝트에서 **Vertex AI API** 사용 설정
3. **API 및 서비스 → OAuth 동의 화면** 설정
   - 사용자 유형: 외부(External)
   - 테스트 사용자에 본인 이메일 추가
4. **API 및 서비스 → 사용자 인증 정보 → OAuth 2.0 클라이언트 ID 만들기**
   - 애플리케이션 유형: 웹 애플리케이션
   - 승인된 JavaScript 출처: 사용할 도메인 (예: `https://yourdomain.github.io`)
5. **클라이언트 ID** 복사 → `VertexAIOAuth({ clientId: '...' })`에 입력
6. 사용할 Google 계정에 해당 프로젝트의 `roles/aiplatform.user` IAM 역할 부여

---

## PostgreSQL 토큰 저장

서버 사이드 앱(Next.js, Express 등)에서 localStorage 대신 PostgreSQL을 사용할 때:

```sql
CREATE TABLE vertex_ai_connections (
  user_id VARCHAR(255) PRIMARY KEY,
  refresh_token TEXT NOT NULL,
  access_token TEXT,
  token_expires_at BIGINT NOT NULL,
  gcp_project_id VARCHAR(255) NOT NULL,
  region VARCHAR(50) DEFAULT 'global',
  scope TEXT,
  enabled BOOLEAN DEFAULT true,
  connected_at BIGINT NOT NULL,
  updated_at BIGINT NOT NULL
);
```

```javascript
const { createTokenManager } = require('./lib/vertex-ai-oauth-postgresql');
const { Pool } = require('pg');

const pool = new Pool({ connectionString: process.env.DATABASE_URL });
const query = (text, params) => pool.query(text, params);

const manager = createTokenManager({
  query,
  clientId: process.env.GCP_OAUTH_CLIENT_ID,
  clientSecret: process.env.GCP_OAUTH_CLIENT_SECRET,
});

// 크레덴셜 조회 (만료 시 자동 갱신)
const creds = await manager.getValidCredentials(userId);
// → { accessToken, projectId, region } 또는 null

// OAuth 콜백 후 토큰 저장
await manager.storeTokens({
  uid: userId, refreshToken, accessToken, expiresIn, projectId, region,
});

// UI용 상태 조회 (토큰 미노출)
const status = await manager.getStatus(userId);
// → { connected: true, minutesLeft: 42 }

// 연결 해제 (Google 폐기 + DB 삭제)
await manager.disconnect(userId);
```

토큰은 PostgreSQL에 평문 저장 (서버 전용, 클라이언트 미노출). 모든 API 비용은 유저 자신의 GCP 프로젝트에 청구됩니다.

---

## 보안 주의사항

- OAuth 액세스 토큰이 `localStorage`에 평문으로 저장됩니다
- 신뢰할 수 없는 서드파티 스크립트와 함께 사용하지 마세요 (XSS 위험)
- 토큰 스코프가 `https://www.googleapis.com/auth/cloud-platform`으로 광범위합니다 — 취급에 주의하세요
- 토큰은 기본 1시간 후 만료되며, 자동 갱신이 silent하게 처리됩니다
- `clientSecret`은 서버 환경 전용입니다 — **절대 브라우저 코드에 포함하지 마세요**

---

## 라이선스

[커스텀 라이선스](./LICENSE) © 2026 shittim-plana  
상업적 이용은 사전 허락이 필요합니다. 자세한 내용은 [LICENSE](./LICENSE)를 참고하세요.
