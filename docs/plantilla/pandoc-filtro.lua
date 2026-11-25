-- pandoc-filtro.lua — Ajustes de la conversión LaTeX -> DOCX (lo usa build-docs.sh).
-- 1) Un longtable con \endfirsthead y \endhead hace que pandoc repita la fila de
--    encabezado como primera fila del cuerpo: se elimina esa fila duplicada.
local function texto_fila(row)
  local partes = {}
  for _, cell in ipairs(row.cells) do
    partes[#partes + 1] = pandoc.utils.stringify(cell.contents)
  end
  return table.concat(partes, '\31')
end

function Table(tbl)
  local filas_cab = tbl.head.rows
  if #filas_cab == 1 and #tbl.bodies >= 1 then
    local cuerpo = tbl.bodies[1].body
    if #cuerpo >= 1 and texto_fila(cuerpo[1]) == texto_fila(filas_cab[1]) then
      table.remove(cuerpo, 1)
    end
  end
  return tbl
end
