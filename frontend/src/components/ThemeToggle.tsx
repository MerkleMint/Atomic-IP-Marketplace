import React from 'react';
import { Sun, Moon } from 'lucide-react';
import { useTheme } from '../context/ThemeContext';

export const ThemeToggle: React.FC = () => {
  const { theme, toggleTheme } = useTheme();

  return (
    <button
      onClick={toggleTheme}
      className="fixed top-4 right-4 p-2 rounded-full bg-primary/10 hover:bg-primary/20 transition-all duration-300 border border-primary/20 group z-50"
      aria-label="Toggle theme"
    >
      <div className="relative w-6 h-6 overflow-hidden">
        <div
          className={`absolute inset-0 transition-transform duration-500 transform ${
            theme === 'dark' ? '-translate-y-full opacity-0' : 'translate-y-0 opacity-100'
          }`}
        >
          <Sun className="w-6 h-6 text-orange-500 group-hover:rotate-12 transition-transform" />
        </div>
        <div
          className={`absolute inset-0 transition-transform duration-500 transform ${
            theme === 'dark' ? 'translate-y-0 opacity-100' : 'translate-y-full opacity-0'
          }`}
        >
          <Moon className="w-6 h-6 text-blue-400 group-hover:-rotate-12 transition-transform" />
        </div>
      </div>
    </button>
  );
};
