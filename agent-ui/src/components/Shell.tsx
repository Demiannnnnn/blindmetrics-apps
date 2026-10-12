import React from 'react';
import { Header } from './Header';
import { Sidebar } from './Sidebar';
import { Banner } from './Banner';
import { useApp } from '../context/AppContext';
import { AttendanceView } from '../views/AttendanceView';
import { RecordsView } from '../views/RecordsView';
import { TelemetryView } from '../views/TelemetryView';
import { MyDataView } from '../views/MyDataView';
import { SettingsView } from '../views/SettingsView';

export const Shell: React.FC = () => {
  const { currentView } = useApp();

  const renderCurrentView = () => {
    switch (currentView) {
      case 'attendance':
        return <AttendanceView />;
      case 'records':
        return <RecordsView />;
      case 'telemetry':
        return <TelemetryView />;
      case 'my-data':
        return <MyDataView />;
      case 'settings':
        return <SettingsView />;
      default:
        return <AttendanceView />;
    }
  };

  return (
    <div className="h-screen w-screen flex flex-col bg-bg text-fg overflow-hidden">
      <Header />
      <Banner />
      <div className="flex-1 flex overflow-hidden">
        <Sidebar />
        <main className="flex-1 overflow-y-auto bg-surface/50">
          {renderCurrentView()}
        </main>
      </div>
    </div>
  );
};
