// Avisos entre pantallas sin sondeo: quien cambia el stock (una venta, un ajuste, una compra)
// avisa, y la barra lateral vuelve a contar los productos bajo el mínimo.

const STOCK = "chups:stock";

export function avisarCambioStock() {
  window.dispatchEvent(new Event(STOCK));
}

/** Devuelve la función para dejar de escuchar. */
export function escucharCambioStock(alCambiar: () => void): () => void {
  window.addEventListener(STOCK, alCambiar);
  return () => window.removeEventListener(STOCK, alCambiar);
}
