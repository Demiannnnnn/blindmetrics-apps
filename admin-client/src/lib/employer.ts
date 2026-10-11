import type { Address } from '../api/types';

export const CHILEAN_REGIONS = [
  'Arica y Parinacota',
  'Tarapacá',
  'Antofagasta',
  'Atacama',
  'Coquimbo',
  'Valparaíso',
  'Región Metropolitana de Santiago',
  "Libertador General Bernardo O'Higgins",
  'Maule',
  'Ñuble',
  'Biobío',
  'La Araucanía',
  'Los Ríos',
  'Los Lagos',
  'Aysén del General Carlos Ibáñez del Campo',
  'Magallanes y de la Antártica Chilena',
] as const;

export function formatAddress(addr: Address): string {
  const parts: string[] = [];
  if (addr.street && addr.number) {
    let main = `${addr.street} ${addr.number}`;
    if (addr.floor) main += `, Piso ${addr.floor}`;
    if (addr.office) main += `, Of. ${addr.office}`;
    parts.push(main);
  }
  if (addr.commune) parts.push(addr.commune);
  if (addr.city && addr.city !== addr.commune) parts.push(addr.city);
  if (addr.region) parts.push(addr.region);
  return parts.join(', ');
}
