// A narrow neck keeps the menu-bar wings clear. Below it, the same outline
// expands into the panel. The command sequence stays identical for interpolation.
export function notchShape(width: number, height: number, neck: number, expanded: boolean): string {
  const left = (width-neck)/2, right = (width+neck)/2;
  const shoulder = expanded ? 18 : 0;
  const spread = expanded ? 16 : 0;
  const corner = expanded ? 22 : 0;
  const bottom = expanded ? 23 : Math.min(12,height/2);
  return `M ${left} 0 L ${right} 0 C ${right} ${shoulder} ${right} ${shoulder} ${right+spread} ${shoulder} L ${width-corner} ${shoulder} Q ${width} ${shoulder} ${width} ${shoulder+corner} L ${width} ${height-bottom} Q ${width} ${height} ${width-bottom} ${height} L ${bottom} ${height} Q 0 ${height} 0 ${height-bottom} L 0 ${shoulder+corner} Q 0 ${shoulder} ${corner} ${shoulder} L ${left-spread} ${shoulder} C ${left} ${shoulder} ${left} ${shoulder} ${left} 0 Z`;
}
