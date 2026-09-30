import { getCurrentWindow } from '@tauri-apps/api/window';
import { Minus, Square, X } from 'lucide-react';

export function TitleBar() {
  const appWindow = getCurrentWindow();

  return (
    <div
      data-tauri-drag-region
      className="h-10 select-none flex justify-between items-center bg-zinc-950/80 backdrop-blur border-b border-white/5 sticky top-0 z-50"
    >
      <div className="pl-4 flex items-center gap-2 pointer-events-none text-zinc-400 font-medium text-sm">
        <img src="/logo.png" alt="Nirakar Chat" className="h-6 w-6 rounded" />
        Nirakar Chat
      </div>
      
      <div className="flex h-full">
        <button
          className="h-full px-4 inline-flex justify-center items-center text-zinc-400 hover:bg-white/10 hover:text-white transition-colors"
          onClick={() => appWindow.minimize()}
        >
          <Minus className="w-4 h-4" />
        </button>
        <button
          className="h-full px-4 inline-flex justify-center items-center text-zinc-400 hover:bg-white/10 hover:text-white transition-colors"
          onClick={() => appWindow.toggleMaximize()}
        >
          <Square className="w-4 h-4" />
        </button>
        <button
          className="h-full px-4 inline-flex justify-center items-center text-zinc-400 hover:bg-red-500 hover:text-white transition-colors"
          onClick={() => appWindow.close()}
        >
          <X className="w-4 h-4" />
        </button>
      </div>
    </div>
  );
}
