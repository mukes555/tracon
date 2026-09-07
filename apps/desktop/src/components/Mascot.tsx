/// An empty-state illustration. The PNGs carry a baked light shadow, so the
/// wrapper gives dark mode a soft disc behind the figure that makes the
/// grey halo read as ground instead of an artifact.
export function Mascot(props: { name: string; className?: string }) {
  return (
    <span className={props.className ? `mascot-wrap ${props.className}` : "mascot-wrap"}>
      <img className="mascot" src={`/mascot/${props.name}.png`} alt="" />
    </span>
  );
}
