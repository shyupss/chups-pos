import Inicio from "./pantallas/Inicio";

export default function App() {
  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center justify-between border-b border-slate-200 bg-white px-6 py-3">
        <h1 className="text-lg font-semibold">CHUPS POS</h1>
        <span className="text-sm text-slate-500">Fase 0 · base del proyecto</span>
      </header>
      <main className="flex-1 overflow-auto p-6">
        <Inicio />
      </main>
    </div>
  );
}
