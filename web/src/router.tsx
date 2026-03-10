import { createBrowserRouter, Navigate } from 'react-router-dom'
import Layout from '@/components/layout/Layout'
import TeamsPage from '@/pages/TeamsPage'
import TeamDetailPage from '@/pages/TeamDetailPage'
import DashboardPage from '@/pages/DashboardPage'
import AgentsPage from '@/pages/AgentsPage'
import AgentDetailPage from '@/pages/AgentDetailPage'
import SkillsPage from '@/pages/SkillsPage'
import McpPage from '@/pages/McpPage'
import ConfigPage from '@/pages/ConfigPage'
import LogsPage from '@/pages/LogsPage'

export const router = createBrowserRouter([
  {
    element: <Layout />,
    children: [
      { index: true, element: <Navigate to="/teams" replace /> },
      { path: '/teams', element: <TeamsPage /> },
      { path: '/teams/:id', element: <TeamDetailPage /> },
      { path: '/dashboard', element: <DashboardPage /> },
      { path: '/agents', element: <AgentsPage /> },
      { path: '/agents/:id', element: <AgentDetailPage /> },
      { path: '/skills', element: <SkillsPage /> },
      { path: '/mcp', element: <McpPage /> },
      { path: '/config', element: <ConfigPage /> },
      { path: '/logs', element: <LogsPage /> },
    ],
  },
])
