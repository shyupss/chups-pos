interface Props {
  etiqueta: string;
  valor: string | number;
}

export default function Dato({ etiqueta, valor }: Props) {
  return (
    <div className="rounded-lg border border-slate-200 bg-white p-4">
      <div className="text-xs uppercase tracking-wide text-slate-500">{etiqueta}</div>
      <div className="mt-1 truncate font-medium" title={String(valor)}>
        {valor}
      </div>
    </div>
  );
}
