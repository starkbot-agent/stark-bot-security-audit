import { Routes, Route } from 'react-router-dom';
import Layout from '@/components/layout/Layout';
import Login from '@/pages/Login';
import Dashboard from '@/pages/Dashboard';
import AgentChat from '@/pages/AgentChat';
import AgentSettings from '@/pages/AgentSettings';
import Channels from '@/pages/Channels';
import Tools from '@/pages/Tools';
import Skills from '@/pages/Skills';
import Sessions from '@/pages/Sessions';
import Memories from '@/pages/Memories';
import Identities from '@/pages/Identities';
import Logs from '@/pages/Logs';
import Debug from '@/pages/Debug';

function App() {
  return (
    <Routes>
      <Route path="/" element={<Login />} />
      <Route element={<Layout />}>
        <Route path="/dashboard" element={<Dashboard />} />
        <Route path="/agent-chat" element={<AgentChat />} />
        <Route path="/agent-settings" element={<AgentSettings />} />
        <Route path="/channels" element={<Channels />} />
        <Route path="/tools" element={<Tools />} />
        <Route path="/skills" element={<Skills />} />
        <Route path="/sessions" element={<Sessions />} />
        <Route path="/memories" element={<Memories />} />
        <Route path="/identities" element={<Identities />} />
        <Route path="/logs" element={<Logs />} />
        <Route path="/debug" element={<Debug />} />
      </Route>
    </Routes>
  );
}

export default App;
