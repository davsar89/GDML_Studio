#!/usr/bin/env python3
"""Small CAD fixtures with independent analytic truth, no importer-derived truth.

Usage: python generate.py /absolute/path/outside/the/repository
Tested with Python 3.12, cadquery-ocp 7.8.1.1.post1, numpy 2.3.5.
The fixture generator is development tooling, never an application dependency.
"""
import argparse
import itertools
import json
import math
from pathlib import Path
import struct

import numpy as np
from OCP.BRep import BRep_Builder, BRep_Tool
from OCP.BRepAlgoAPI import BRepAlgoAPI_Cut, BRepAlgoAPI_Fuse
from OCP.BRepBuilderAPI import (BRepBuilderAPI_MakeEdge, BRepBuilderAPI_MakeFace,
                               BRepBuilderAPI_MakePolygon, BRepBuilderAPI_MakeSolid,
                               BRepBuilderAPI_MakeWire, BRepBuilderAPI_Sewing,
                               BRepBuilderAPI_Transform)
from OCP.BRepFilletAPI import BRepFilletAPI_MakeFillet
from OCP.BRepLib import BRepLib
from OCP.BRepMesh import BRepMesh_IncrementalMesh
from OCP.BRepPrimAPI import (BRepPrimAPI_MakeBox, BRepPrimAPI_MakeCone,
                            BRepPrimAPI_MakeCylinder, BRepPrimAPI_MakePrism,
                            BRepPrimAPI_MakeRevol, BRepPrimAPI_MakeSphere,
                            BRepPrimAPI_MakeTorus)
from OCP.GC import GC_MakeArcOfCircle
from OCP.Geom import Geom_BezierSurface
from OCP.GeomConvert import GeomConvert
from OCP.IFSelect import IFSelect_RetDone
from OCP.Interface import Interface_Static
from OCP.STEPControl import STEPControl_Writer, STEPControl_AsIs
from OCP.STEPCAFControl import STEPCAFControl_Writer
from OCP.TCollection import TCollection_ExtendedString
from OCP.TColgp import TColgp_Array2OfPnt
from OCP.TDataStd import TDataStd_Name
from OCP.TDF import TDF_LabelSequence
from OCP.TDocStd import TDocStd_Document
from OCP.TopAbs import TopAbs_EDGE, TopAbs_FACE, TopAbs_REVERSED
from OCP.TopExp import TopExp_Explorer
from OCP.TopLoc import TopLoc_Location
from OCP.TopoDS import TopoDS, TopoDS_Compound
from OCP.XCAFDoc import XCAFDoc_DocumentTool
from OCP.gp import gp_Ax1, gp_Ax2, gp_Dir, gp_Pnt, gp_Trsf, gp_Vec


def box(x, y, z, origin=None):
    return BRepPrimAPI_MakeBox(gp_Pnt(*(origin or (-x/2, -y/2, -z/2))), x, y, z).Shape()


def cyl(r, h, x=0, y=0, z=None, angle=2*math.pi):
    return BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(x,y,-h/2 if z is None else z),
                                         gp_Dir(0,0,1)),r,h,angle).Shape()


def cone(r1, r2, z1, z2):
    """Frustum from radius r1 at z1 to r2 at z2 about the z axis."""
    return BRepPrimAPI_MakeCone(gp_Ax2(gp_Pnt(0,0,z1),gp_Dir(0,0,1)),r1,r2,z2-z1).Shape()


def oblique(shape):
    """`shape` turned about a non-principal axis and moved off the origin."""
    t=gp_Trsf();t.SetRotation(gp_Ax1(gp_Pnt(),gp_Dir(1,2,3)),0.7);t.SetTranslationPart(gp_Vec(30,-20,10))
    return BRepBuilderAPI_Transform(shape,t,True).Shape()


def face(points):
    w = BRepBuilderAPI_MakePolygon()
    for p in points:
        w.Add(gp_Pnt(*p))
    w.Close()
    return BRepBuilderAPI_MakeFace(w.Wire()).Face()


def write_step(shape, path, unit='MM'):
    writer = STEPControl_Writer()
    Interface_Static.SetCVal_s('write.step.unit', unit)
    # Both the declared unit and coordinate conversion must be set. Changing
    # only the global label can leave millimetre coordinates labelled INCH.
    writer.Model().SetLocalLengthUnit(1.0)
    writer.Model().SetWriteLengthUnit(25.4 if unit == 'INCH' else 1.0)
    writer.Transfer(shape, STEPControl_AsIs)
    assert writer.Write(str(path)) == IFSelect_RetDone
    Interface_Static.SetCVal_s('write.step.unit', 'MM')


def mesh(shape, chord=0.02, angle=0.2):
    BRepMesh_IncrementalMesh(shape, chord, False, angle, False)
    tris = []
    exp = TopExp_Explorer(shape, TopAbs_FACE)
    while exp.More():
        f = TopoDS.Face_s(exp.Current())
        loc = TopLoc_Location()
        tri = BRep_Tool.Triangulation_s(f, loc)
        if tri is not None:
            pts = [tri.Node(i).Transformed(loc.Transformation()) for i in range(1,tri.NbNodes()+1)]
            for i in range(1, tri.NbTriangles()+1):
                ids = list(tri.Triangle(i).Get())
                if f.Orientation() == TopAbs_REVERSED:
                    ids[1], ids[2] = ids[2], ids[1]
                tris.append([[pts[j-1].X(),pts[j-1].Y(),pts[j-1].Z()] for j in ids])
        exp.Next()
    return np.asarray(tris)


def stl_binary(path, tris, header=b'GDML Studio fixture'):
    with path.open('wb') as f:
        f.write(header.ljust(80,b' ')[:80]); f.write(struct.pack('<I',len(tris)))
        for t in tris:
            # Normals intentionally zero: winding, not the stored normal, is truth.
            f.write(struct.pack('<12fH',0,0,0,*np.asarray(t).ravel(),0))


def stl_ascii(path, blocks):
    out=[]
    for name,tris in blocks:
        out.append('solid '+name)
        for t in tris:
            out.extend(['facet normal 0 0 0','outer loop'])
            out.extend('vertex '+' '.join(format(float(x),'.17g') for x in p) for p in t)
            out.extend(['endloop','endfacet'])
        out.append('endsolid '+name)
    path.write_text('\n'.join(out)+'\n')


def main(out):
    out.mkdir(parents=True,exist_ok=True)
    expected={}
    shapes={}

    # `family` is what the importer must make of it (doc/cad-import/PLAN.md,
    # algorithm D): a GDML solid, "<stock> - <n> cut-outs", or "tessellated".
    def add(name, shape, volume, bbox, formula, family=None, **extra):
        shapes[name]=shape
        expected[name]={'volume_mm3':volume,'bbox_mm':bbox,'formula':formula,'family':family,**extra}
        write_step(shape,out/(name+'.step'))

    add('box',box(10,20,30),6000,[-5,-10,-15,5,10,15],'10*20*30',
        family='box')
    add('tube',BRepAlgoAPI_Cut(cyl(10,20),cyl(8,22)).Shape(),720*math.pi,
        [-10,-10,-10,10,10,10],'pi*(10^2-8^2)*20',
        family='tube')
    sector=BRepAlgoAPI_Cut(cyl(10,12,angle=1.5*math.pi),cyl(4,14)).Shape()
    add('tube_sector',sector,756*math.pi,[-10,-10,-6,10,10,6],
        '(270/360)*pi*(10^2-4^2)*12',
        family='tube')
    add('cone',cone(10,5,-10,10),3500*math.pi/3,[-10,-10,-10,10,10,10],
        'pi*20*(10^2+10*5+5^2)/3',
        family='cone')
    add('sphere',BRepPrimAPI_MakeSphere(10).Shape(),4000*math.pi/3,
        [-10,-10,-10,10,10,10],'4*pi*10^3/3',
        family='orb')
    add('torus',BRepPrimAPI_MakeTorus(20,4).Shape(),640*math.pi**2,
        [-24,-24,-4,24,24,4],'2*pi^2*20*4^2',
        family='torus')
    profile=face([(0,0,-10),(10,0,-10),(10,0,0),(5,0,10),(0,0,10)])
    poly=BRepPrimAPI_MakeRevol(profile,gp_Ax1(gp_Pnt(),gp_Dir(0,0,1)),2*math.pi).Shape()
    add('polycone',poly,math.pi*(1000+1750/3),[-10,-10,-10,10,10,10],
        'pi*(10^2*10 + 10*(10^2+10*5+5^2)/3)',
        family='genericPolycone')
    polygon=[(0,0),(20,0),(20,5),(5,5),(5,15),(0,15)]
    extrusion=BRepPrimAPI_MakePrism(face([(x,y,-5) for x,y in polygon]),gp_Vec(0,0,10)).Shape()
    add('l_extrusion',extrusion,1500,[0,0,-5,20,15,5],'(20*5+5*10)*10',
        family='xtru')
    plate=box(40,30,10)
    for x in (-10,10):
        plate=BRepAlgoAPI_Cut(plate,cyl(3,12,x=x)).Shape()
    plate=BRepAlgoAPI_Cut(plate,cyl(5,4,x=-10,z=2)).Shape()
    add('hole_plate',plate,12000-228*math.pi,[-20,-15,-5,20,15,5],
        '40*30*10 - 2*pi*3^2*10 - pi*(5^2-3^2)*3',
        family='box - 2 cut-outs')
    # Edges must belong to the same input shape.
    original=box(20,16,12)
    fillet=BRepFilletAPI_MakeFillet(original)
    ex=TopExp_Explorer(original,TopAbs_EDGE)
    while ex.More():
        fillet.Add(2,TopoDS.Edge_s(ex.Current())); ex.Next()
    fillet.Build(); assert fillet.IsDone()
    r=2; a,b,c=16,12,8
    rounded=a*b*c+2*r*(a*b+a*c+b*c)+math.pi*r*r*(a+b+c)+4*math.pi*r**3/3
    add('filleted_block',fillet.Shape(),rounded,[-10,-8,-6,10,8,6],
        'a*b*c+2*r*(a*b+a*c+b*c)+pi*r^2*(a+b+c)+4*pi*r^3/3; a=16,b=12,c=8,r=2',
        family='tessellated')
    # Exact biquadratic roof, z=10+3*(1-(x/10)^2)*(1-(y/8)^2).
    # Integral of the product bump is area*4/9. The central pole is not the max.
    poles=TColgp_Array2OfPnt(1,3,1,3)
    for i,x in enumerate((-10,0,10),1):
        for j,y in enumerate((-8,0,8),1):
            poles.SetValue(i,j,gp_Pnt(x,y,22 if (i,j)==(2,2) else 10))
    surf=GeomConvert.SurfaceToBSplineSurface_s(Geom_BezierSurface(poles))
    sewing=BRepBuilderAPI_Sewing(1e-7)
    sewing.Add(BRepBuilderAPI_MakeFace(surf,1e-7).Face())
    corners=[(-10,-8),(10,-8),(10,8),(-10,8)]
    sewing.Add(face([(x,y,0) for x,y in reversed(corners)]))
    for i in range(4):
        p,q=corners[i],corners[(i+1)%4]
        sewing.Add(face([(*p,0),(*q,0),(*q,10),(*p,10)]))
    sewing.Perform()
    roof=BRepBuilderAPI_MakeSolid(TopoDS.Shell_s(sewing.SewedShape())).Solid()
    assert BRepLib.OrientClosedSolid_s(roof)
    add('bspline_roof',roof,320*(10+4*3/9),[-10,-8,0,10,8,13],
        'area*(H+4*A/9); area=20*16,H=10,A=3',
        family='tessellated')
    add('closed_cavity',BRepAlgoAPI_Cut(box(10,10,10),box(6,6,6)).Shape(),784,
        [-5,-5,-5,5,5,5],'10^3-6^3',
        family='box - 1 cut-out')
    add('inch_box',box(25.4,50.8,76.2),6*25.4**3,
        [-12.7,-25.4,-38.1,12.7,25.4,38.1],'1*2*3*25.4^3',
        family='box')
    write_step(shapes['inch_box'],out/'inch_box.step','INCH')

    # True XCAF assembly with shared part definition and nested assembly.
    doc=TDocStd_Document(TCollection_ExtendedString('BinXCAF'))
    st=XCAFDoc_DocumentTool.ShapeTool_s(doc.Main())
    root=st.NewShape(); TDataStd_Name.Set_s(root,TCollection_ExtendedString('assembly'))
    sub=st.NewShape(); TDataStd_Name.Set_s(sub,TCollection_ExtendedString('nested'))
    part=st.AddShape(shapes['box'],False)
    TDataStd_Name.Set_s(part,TCollection_ExtendedString('part 0x bad & name'))
    transforms=[]
    for x,angle in ((-40,0),(40,37)):
        t=gp_Trsf();t.SetRotation(gp_Ax1(gp_Pnt(),gp_Dir(1,2,3)),math.radians(angle))
        t.SetTranslationPart(gp_Vec(x,0,0))
        st.AddComponent(sub,part,TopLoc_Location(t))
        transforms.append([[t.Value(i,j) for j in range(1,5)] for i in range(1,4)])
    outer=gp_Trsf();outer.SetRotation(gp_Ax1(gp_Pnt(),gp_Dir(2,1,3)),math.radians(23))
    outer.SetTranslationPart(gp_Vec(0,20,0))
    st.AddComponent(root,sub,TopLoc_Location(outer))
    # STEP writers commonly materialize a reflected part as another definition.
    mirror=gp_Trsf();mirror.SetMirror(gp_Ax2(gp_Pnt(),gp_Dir(1,0,0)))
    mirrored=BRepBuilderAPI_Transform(extrusion,mirror,True).Shape()
    ml=st.AddShape(mirrored,False)
    TDataStd_Name.Set_s(ml,TCollection_ExtendedString('mirrored_L'))
    mt=gp_Trsf();mt.SetTranslation(gp_Vec(0,-40,0));st.AddComponent(root,ml,TopLoc_Location(mt))
    st.UpdateAssemblies()
    writer=STEPCAFControl_Writer();writer.SetNameMode(True)
    assert writer.Transfer(doc,STEPControl_AsIs)
    assert writer.Write(str(out/'assembly.step'))==IFSelect_RetDone
    outer_m=np.array([[outer.Value(i,j) for j in range(1,5)] for i in range(1,4)])
    pts=[];world_mats=[]
    for t in transforms:
        tm=np.eye(4);tm[:3]=t;om=np.eye(4);om[:3]=outer_m;wm=om@tm
        world_mats.append(wm.tolist())
        pts.extend((wm@np.array([x,y,z,1]))[:3] for x,y,z in itertools.product((-5,5),(-10,10),(-15,15)))
    pts.extend([(-x,y-40,z) for x,y in polygon for z in (-5,5)])
    pts=np.asarray(pts)
    expected['assembly']={'volume_mm3':13500,'bbox_mm':[*pts.min(0),*pts.max(0)],
                          'formula':'2*(10*20*30)+1500', 'box_world_matrices':world_mats,
                          'mirrored_L_matrix':[[-1,0,0,0],[0,1,0,-40],[0,0,1,0],[0,0,0,1]],
                          'leaf_occurrences':3,'unique_definitions':2}

    tris=mesh(shapes['box'])
    stl_binary(out/'box_binary.stl',tris)
    stl_binary(out/'binary_solid_header.stl',tris,b'solid misleading_binary_header')
    stl_ascii(out/'box_ascii.stl',[('box',tris)])
    stl_ascii(out/'multi_solid.stl',[('one',tris),('two',tris+[30,0,0])])
    stl_binary(out/'unwelded.stl',tris)
    stl_binary(out/'open.stl',tris[:-1])
    flipped=tris.copy();flipped[0]=flipped[0][::-1]
    stl_binary(out/'one_flipped.stl',flipped)
    stl_binary(out/'all_flipped.stl',tris[:,::-1])
    stl_binary(out/'with_degenerate.stl',np.concatenate([tris,[[tris[0,0]]*3]]))
    for name in ('tube','sphere','torus','closed_cavity','bspline_roof'):
        stl_binary(out/(name+'.stl'),mesh(shapes[name]))
    # The clearance/overlap examples use exact concentric cylinders.
    add('shaft',cyl(8,20),1280*math.pi,[-8,-8,-10,8,8,10],'pi*8^2*20',
        family='tube')
    add('interfering_shaft',cyl(8.1,20),20*math.pi*8.1**2,
        [-8.1,-8.1,-10,8.1,8.1,10],'pi*8.1^2*20',
        family='tube')

    # Recognition: families placed obliquely, faces split at seams, and
    # lookalikes that must not become the family they resemble.
    add('oblique_cone',oblique(BRepAlgoAPI_Cut(cone(10,6,-10,10),cone(4.1,1.9,-11,11)).Shape()),
        1120*math.pi,None,'pi*20/3*((10^2+10*6+6^2)-(4^2+4*2+2^2))',family='cone')
    t0,t1,rmin,rmax,dphi=0.5,2.0,5,10,4.0
    def polar(r,t): return gp_Pnt(r*math.sin(t),0,r*math.cos(t))
    arc=lambda r: BRepBuilderAPI_MakeEdge(GC_MakeArcOfCircle(polar(r,t0),polar(r,(t0+t1)/2),polar(r,t1)).Value()).Edge()
    wire=BRepBuilderAPI_MakeWire(arc(rmax),BRepBuilderAPI_MakeEdge(polar(rmax,t1),polar(rmin,t1)).Edge(),
                                 arc(rmin),BRepBuilderAPI_MakeEdge(polar(rmin,t0),polar(rmax,t0)).Edge()).Wire()
    shell=BRepPrimAPI_MakeRevol(BRepBuilderAPI_MakeFace(wire,True).Face(),gp_Ax1(gp_Pnt(),gp_Dir(0,0,1)),dphi).Shape()
    add('oblique_sphere_shell',oblique(shell),dphi/3*(rmax**3-rmin**3)*(math.cos(t0)-math.cos(t1)),None,
        'dphi/3*(10^3-5^3)*(cos 0.5-cos 2); dphi=4',family='sphere')
    torus=BRepAlgoAPI_Cut(BRepPrimAPI_MakeTorus(gp_Ax2(),20,4,1.5*math.pi).Shape(),
                          BRepPrimAPI_MakeTorus(gp_Ax2(),20,2,1.5*math.pi).Shape()).Shape()
    add('oblique_hollow_torus',oblique(torus),1.5*math.pi*math.pi*20*(4**2-2**2),None,
        '1.5pi*pi*20*(4^2-2^2)',family='torus')
    half=cyl(10,20,angle=math.pi)
    turn=gp_Trsf();turn.SetRotation(gp_Ax1(gp_Pnt(),gp_Dir(0,0,1)),math.pi)
    halves=BRepAlgoAPI_Fuse(half,BRepBuilderAPI_Transform(half,turn,True).Shape()).Shape()
    add('seam_split_tube',BRepAlgoAPI_Cut(halves,cyl(8,22)).Shape(),720*math.pi,
        [-10,-10,-10,10,10,10],'pi*(10^2-8^2)*20, built from two halves',family='tube')
    add('eccentric_bore',BRepAlgoAPI_Cut(cyl(10,20),cyl(8,22,x=0.01)).Shape(),720*math.pi,
        [-10,-10,-10,10,10,10],'pi*(10^2-8^2)*20, bore 0.01 off the axis',family='tessellated')
    k=math.tan(math.radians(0.1)); r2=8-20*k
    add('tapered_bore',BRepAlgoAPI_Cut(cyl(10,20),cone(8+k,8-21*k,-11,11)).Shape(),
        math.pi*(100*20-20/3*(64+8*r2+r2*r2)),[-10,-10,-10,10,10,10],
        'pi*(10^2*20 - 20/3*(8^2+8*r2+r2^2)), r2=8-20*tan(0.1 deg)',family='cone')

    # Bounded CSG: a stock minus up to 16 cut-outs.
    flange=BRepAlgoAPI_Cut(cyl(30,6),cyl(5,8)).Shape()
    for i in range(6):
        a=math.radians(60*i)
        flange=BRepAlgoAPI_Cut(flange,cyl(2.5,8,x=20*math.cos(a),y=20*math.sin(a))).Shape()
    add('flange',flange,math.pi*((30**2-5**2)*6-6*2.5**2*6),[-30,-30,-3,30,30,3],
        'pi*((30^2-5^2)*6 - 6*2.5^2*6)',family='tube - 6 cut-outs')
    # The pocket is turned and off-centre, so a wrong operand rotation shows.
    turn=gp_Trsf();turn.SetRotation(gp_Ax1(gp_Pnt(),gp_Dir(0,0,1)),math.radians(30));turn.SetTranslationPart(gp_Vec(5,2,0))
    pocket=BRepBuilderAPI_Transform(box(20,10,5,origin=(-10,-5,1)),turn,True).Shape()
    add('pocket_block',BRepAlgoAPI_Cut(box(40,30,10),pocket).Shape(),12000-20*10*4,[-20,-15,-5,20,15,5],
        '40*30*10 - 20*10*4, the pocket turned 30 deg about z',family='box - 1 cut-out')
    shifted=box(40,30,10)
    for x in (-10,10):
        shifted=BRepAlgoAPI_Cut(shifted,cyl(3,12,x=x)).Shape()
    shifted=BRepAlgoAPI_Cut(shifted,cyl(5,4,x=-10,z=2.001)).Shape()
    add('counterbore_shift',shifted,12000-math.pi*(2*9*10+16*2.999),[-20,-15,-5,20,15,5],
        '40*30*10 - pi*(2*3^2*10 + (5^2-3^2)*2.999)',family='box - 2 cut-outs')
    holes=box(100,100,5)
    for i in range(17):
        holes=BRepAlgoAPI_Cut(holes,cyl(1,7,x=-40+5*i)).Shape()
    add('plate_17_holes',holes,50000-17*math.pi*5,[-50,-50,-2.5,50,50,2.5],'100*100*5 - 17*pi*1^2*5',
        family='tessellated')

    # A shaft in a bore of its radius: two solids that touch without interfering.
    pair=TopoDS_Compound();builder=BRep_Builder();builder.MakeCompound(pair)
    for solid in (shapes['tube'],shapes['shaft']):
        builder.Add(pair,solid)
    write_step(pair,out/'shaft_in_bore.step')
    expected['shaft_in_bore']={'intersection_mm3':0,'formula':'equal radii, boundary contact only'}
    expected['interference']={'intersection_mm3':20*math.pi*(8.1**2-8**2),
                              'formula':'pi*(8.1^2-8^2)*20'}
    (out/'expected.json').write_text(json.dumps(expected,indent=2)+'\n')
    print('Fixture truth uses analytic formulas, not kernel mass properties.')


if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('output',type=Path)
    main(ap.parse_args().output.resolve())
