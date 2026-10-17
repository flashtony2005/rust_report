import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'

import App from './App'
import AuthGate from './AuthGate'

createRoot(document.getElementById('app')!).render(
  <StrictMode>
    <AuthGate>
      <App />
    </AuthGate>
  </StrictMode>,
)
