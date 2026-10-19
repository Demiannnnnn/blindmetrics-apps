import React from 'react';
import {
  Clock,
  FileText,
  Activity,
  User,
  Settings,
} from 'lucide-react';
import { useApp, type NavView } from '../context/AppContext';

interface NavItem {
  id: NavView;
  label: string;
  icon: React.ComponentType<{ className?: string; 'aria-hidden'?: boolean | 'true' | 'false' }>;
}

const NAV_ITEMS: NavItem[] = [
  { id: 'attendance', label: 'Asistencia', icon: Clock },
  { id: 'records', label: 'Registros', icon: FileText },
  { id: 'telemetry', label: 'Telemetría', icon: Activity },
  { id: 'my-data', label: 'Mis datos', icon: User },
  { id: 'settings', label: 'Ajustes', icon: Settings },
];

export const Sidebar: React.FC = () => {
  const { currentView, setCurrentView } = useApp();

  const handleKeyDown = (e: React.KeyboardEvent, index: number) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      const next = (index + 1) % NAV_ITEMS.length;
      setCurrentView(NAV_ITEMS[next].id);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      const prev = (index - 1 + NAV_ITEMS.length) % NAV_ITEMS.length;
      setCurrentView(NAV_ITEMS[prev].id);
    }
  };

  return (
    <nav
      className="w-20 bg-ink border-r border-ink-line flex flex-col items-center py-3 select-none shrink-0"
      aria-label="Navegación principal"
    >
      <div className="w-full space-y-1">
        {NAV_ITEMS.map((item, index) => {
          const isActive = currentView === item.id;
          const Icon = item.icon;

          return (
            <button
              key={item.id}
              onClick={() => setCurrentView(item.id)}
              onKeyDown={(e) => handleKeyDown(e, index)}
              aria-current={isActive ? 'page' : undefined}
              className={`w-full py-2.5 px-1 flex flex-col items-center justify-center relative transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent ${
                isActive
                  ? 'bg-ink-raised text-ink-fg font-medium'
                  : 'text-ink-muted hover:bg-ink-raised/50 hover:text-ink-fg'
              }`}
            >
              {isActive && (
                <div
                  className="absolute left-0 top-0 bottom-0 w-[2px] bg-brand"
                  aria-hidden="true"
                />
              )}
              <Icon
                className={`w-5 h-5 mb-1 ${isActive ? 'text-brand' : 'text-ink-muted'}`}
                aria-hidden="true"
              />
              <span className="text-[11px] leading-tight text-center tracking-tight">
                {item.label}
              </span>
            </button>
          );
        })}
      </div>
    </nav>
  );
};
