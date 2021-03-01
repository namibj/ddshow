const raw_nodes = {{ nodes | json_encode() }};
const raw_subgraphs = {{ subgraphs | json_encode() }};
const raw_edges = {{ edges | json_encode() }};
const palette_colors = {{ palette_colors | json_encode() }};

const dataflow_svg = d3.select("#dataflow-graph");
const svg = dataflow_svg.append("g");

// Create the zoomable area for the graph
const zoom = d3.zoom().on("zoom", () => {
    svg.attr("transform", d3.event.transform);
});
dataflow_svg.call(zoom);

const graph = new dagreD3.graphlib.Graph({ compound: true });
graph.setGraph({ nodesep: 50, ranksep: 50 });

const render = new dagreD3.render();

for (const node of raw_nodes) {
    const node_id = node.addr.toString();
    graph.setNode(
        node_id,
        {
            label: node.name,
            style: `fill: ${node.fill_color}`,
            labelStyle: `fill: ${node.text_color}`,
            data: { kind: "Node", ...node },
        },
    );

    const parent_addr = node.addr.slice(0, node.addr.length - 1);
    graph.setParent(node_id, parent_addr.toString());
}

for (const subgraph of raw_subgraphs) {
    const subgraph_id = subgraph.addr.toString();
    graph.setNode(
        subgraph_id,
        {
            label: subgraph.name,
            style: "fill: #EEEEEE; stroke-dasharray: 5, 2;",
            clusterLabelPos: "top",
            data: { kind: "Subgraph", ...subgraph },
        },
    );

    if (subgraph.addr.length > 1) {
        const parent_addr = subgraph.addr.slice(0, subgraph.addr.length - 1);
        graph.setParent(subgraph_id, parent_addr.toString());
    }
}

for (const edge of raw_edges) {
    let style = "";
    switch (edge.edge_kind) {
        case "Crossing":
            // Blue
            style = "stroke: #5d5de6; stroke-dasharray: 5, 2; fill: none;"
            break;

        case "Normal":
            break;

        default:
            throw "Invalid edge kind received";
    }

    graph.setEdge(
        edge.src.toString(),
        edge.dest.toString(),
        {
            style: style,
            data: { kind: "Edge", ...edge },
        },
    );
}

// Render the graph
render(svg, graph);

// Create the tooltip div
const tooltip = d3.select("body")
    .append("div")
    .attr("id", "tooltip_template")
    .style("position", "absolute")
    .style("background-color", "white")
    .style("border", "solid")
    .style("display", "block")
    .style("border-width", "2px")
    .style("border-radius", "5px")
    .style("padding", "15px")
    .style("z-index", "10")
    .style("visibility", "hidden")
    .style("font-size", "0.7vw")
    .text("");

// Node tooltips
svg.selectAll("g.node")
    // Create the text to be displayed for each node
    .attr("data-hovertext", node_id => {
        const node = graph.node(node_id).data;
        const text = `ran for ${node.total_activation_time} over ${node.invocations} invocations<br>\
            average runtime of ${node.average_activation_time} \
            (max: ${node.max_activation_time}, min: ${node.min_activation_time})`;

        return text;
    })
    // Reveal the tooltip on hover
    .on("mouseover", () => tooltip.style("visibility", "visible"))
    .on("mousemove", function () {
        tooltip
            .html(this.dataset.hovertext)
            .style("top", (d3.event.pageY - 40) + "px")
            .style("left", (d3.event.pageX + 40) + "px");
    })
    // Hide the tooltip on mouseout
    .on("mouseout", () => tooltip.style("visibility", "hidden"));

// Edge tooltips
svg.selectAll("g.edgePath")
    // Create the text to be displayed for each edge
    .attr("data-hovertext", edge_id => {
        const edge = graph.edge(edge_id).data;
        const src = graph.node(edge.src).data,
            dest = graph.node(edge.dest).data;

        return `channel from ${src.name} to ${dest.name}`;
    })
    // Reveal the tooltip on hover
    .on("mouseover", () => tooltip.style("visibility", "visible"))
    .on("mousemove", function () {
        tooltip
            .html(this.dataset.hovertext)
            .style("top", (d3.event.pageY - 40) + "px")
            .style("left", (d3.event.pageX + 40) + "px");
    })
    // Hide the tooltip on mouseout
    .on("mouseout", () => tooltip.style("visibility", "hidden"));

// Add the palette legend
const palette_legend = d3.select("body")
    .append("div")
    .attr("id", "palette-legend")
    .style("position", "absolute")
    .style("top", "0")
    .style("right", "0")
    .style("background-color", "white")
    .style("border", "solid")
    .style("display", "block")
    .style("border-width", "2px")
    .style("border-radius", "5px")
    .style("padding", "10px")
    .style("z-index", "10")
    .style("margin", "15px")
    .style("font-size", "0.7vw")
    .style("height", "15%")
    .style("width", "3.5%")
    .style("text-align", "center");

// Make the gradient's css
let palette_gradient = "",
    gradient_counter = 0;
for (color of palette_colors) {
    palette_gradient += `${color} ${gradient_counter}%, `;
    gradient_counter += 100 / palette_colors.length;
}
if (palette_gradient.endsWith(", ")) {
    palette_gradient = palette_gradient.substring(0, palette_gradient.length - 2);
}

// Top text
palette_legend
    .append("div")
    .attr("class", "palette-text")
    .text("slower");

// Heatgraph gradient
palette_legend
    .append("div")
    .attr("id", "palette-gradient")
    .style("background", `linear-gradient(to top, ${palette_gradient})`)
    .style("height", "75%")
    .style("width", "100%")
    .style("display", "inline-block");

// Bottom text
palette_legend
    .append("div")
    .attr("class", "palette-text")
    .text("faster");

// Center & scale the graph
const initialScale = 1.00;
d3.zoomIdentity
    .translate([(svg.attr("width") - graph.graph().width * initialScale) / 2, 20])
    .scale(initialScale);
dataflow_svg.attr("height", graph.graph().height * initialScale + 40);
